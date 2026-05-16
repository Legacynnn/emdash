//! `TasksService` — DB + worktree CRUD.
//!
//! Every write path is gated by `WorkspaceFsMutationLock` so concurrent
//! create/destroy calls on the same task id serialize, both for the
//! filesystem operation and the DB row update. The lock is per-task
//! (the task id is the workspace id in v1 since one task = one
//! workspace).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::db::Db;

use super::fs_lock::WorkspaceFsMutationLock;
use super::model::{NewTaskInput, Task, TaskSourceBranch, TaskStatus, TasksError};
use super::worktree;

pub struct TasksService {
    db: Arc<Db>,
    fs_lock: Arc<WorkspaceFsMutationLock>,
}

impl TasksService {
    pub fn new(db: Arc<Db>, fs_lock: Arc<WorkspaceFsMutationLock>) -> Self {
        Self { db, fs_lock }
    }

    pub fn list(&self, project_id: &str) -> Result<Vec<Task>, TasksError> {
        let conn = self.db.read()?;
        let mut stmt = conn.prepare(
            "SELECT id, project_id, name, status, path, source_branch, pty_id, created_at, updated_at \
             FROM tasks \
             WHERE project_id = ? AND archived_at IS NULL \
             ORDER BY created_at DESC, id ASC",
        )?;
        let rows = stmt
            .query_map(params![project_id], row_to_task)?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(decode_row_source_branch).collect()
    }

    pub fn get(&self, task_id: &str) -> Result<Option<Task>, TasksError> {
        let conn = self.db.read()?;
        let row = conn
            .query_row(
                "SELECT id, project_id, name, status, path, source_branch, pty_id, created_at, updated_at \
                 FROM tasks WHERE id = ?",
                params![task_id],
                row_to_task,
            )
            .optional()?;
        match row {
            Some(t) => decode_row_source_branch(t).map(Some),
            None => Ok(None),
        }
    }

    /// Create a task and its worktree. The DB row is written **after**
    /// `git worktree add` succeeds — a partially-created worktree on
    /// the filesystem with no matching DB row is easier to clean up
    /// than the inverse.
    ///
    /// Branch + worktree share a name. If the renderer passes an
    /// explicit `task_branch` it is used verbatim (e.g. `feat/add-x`);
    /// otherwise the name is slugified. A collision on the local
    /// branch name, the remote-tracking name, or the worktree
    /// directory produces `BranchAlreadyExists` so the renderer can
    /// surface a rename prompt.
    pub fn create(&self, input: NewTaskInput) -> Result<Task, TasksError> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err(TasksError::EmptyName);
        }
        let project_path = self.project_path(&input.project_id)?;

        let task_branch = resolve_task_branch(&input.task_branch, name)?;
        if branch_exists(&project_path, &task_branch)? {
            return Err(TasksError::BranchAlreadyExists(task_branch));
        }

        let task_path = worktree::worktree_path(&project_path, &task_branch);
        if task_path.exists() {
            return Err(TasksError::BranchAlreadyExists(task_branch));
        }

        let task_id = Uuid::new_v4().to_string();
        let source = input.source_branch.checkout_target();

        let mutex = self.fs_lock.lock_for(&task_id);
        let _guard = mutex.lock();

        worktree::add(&project_path, &task_path, &task_branch, &source)?;

        let row = self.insert(
            &task_id,
            &input.project_id,
            name,
            &task_path,
            &task_branch,
            &input.source_branch,
        );
        match row {
            Ok(task) => Ok(task),
            Err(insert_err) => {
                // Best-effort rollback so we don't leak a worktree.
                let _ = worktree::remove(&project_path, &task_path);
                Err(insert_err)
            }
        }
    }

    pub fn delete(&self, task_id: &str) -> Result<(), TasksError> {
        let task = self
            .get(task_id)?
            .ok_or_else(|| TasksError::NotFound(task_id.to_string()))?;
        let project_path = self.project_path(&task.project_id)?;
        let task_path = PathBuf::from(&task.path);

        let mutex = self.fs_lock.lock_for(task_id);
        let _guard = mutex.lock();

        // Filesystem first; if the worktree directory is already missing,
        // proceed to the DB cleanup anyway so the user can recover from a
        // half-deleted state.
        let _ = worktree::remove(&project_path, &task_path);

        let conn = self.db.write()?;
        let affected = conn.execute("DELETE FROM tasks WHERE id = ?", params![task_id])?;
        if affected == 0 {
            return Err(TasksError::NotFound(task_id.to_string()));
        }
        Ok(())
    }

    /// Update the user-facing `name` of a task. Does not touch the
    /// underlying git branch; the renderer keeps display name and
    /// branch name as separate concerns.
    pub fn rename(&self, task_id: &str, name: &str) -> Result<Task, TasksError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(TasksError::EmptyName);
        }
        let conn = self.db.write()?;
        let affected = conn.execute(
            "UPDATE tasks SET name = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![trimmed, task_id],
        )?;
        if affected == 0 {
            return Err(TasksError::NotFound(task_id.to_string()));
        }
        drop(conn);
        self.get(task_id)?
            .ok_or_else(|| TasksError::NotFound(task_id.to_string()))
    }

    /// Soft-archive a task. Sets `status = 'archived'` and stamps
    /// `archived_at` so the list view can filter it out. Worktree
    /// stays on disk — restore is non-destructive.
    pub fn archive(&self, task_id: &str) -> Result<Task, TasksError> {
        let conn = self.db.write()?;
        let affected = conn.execute(
            "UPDATE tasks \
             SET status = 'archived', archived_at = CURRENT_TIMESTAMP, \
                 status_changed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
             WHERE id = ?1",
            params![task_id],
        )?;
        if affected == 0 {
            return Err(TasksError::NotFound(task_id.to_string()));
        }
        drop(conn);
        self.get(task_id)?
            .ok_or_else(|| TasksError::NotFound(task_id.to_string()))
    }

    /// Reverse of `archive`. Clears `archived_at` and flips status
    /// back to `active`.
    pub fn restore(&self, task_id: &str) -> Result<Task, TasksError> {
        let conn = self.db.write()?;
        let affected = conn.execute(
            "UPDATE tasks \
             SET status = 'active', archived_at = NULL, \
                 status_changed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
             WHERE id = ?1",
            params![task_id],
        )?;
        if affected == 0 {
            return Err(TasksError::NotFound(task_id.to_string()));
        }
        drop(conn);
        self.get(task_id)?
            .ok_or_else(|| TasksError::NotFound(task_id.to_string()))
    }

    /// Pin / unpin a task. The renderer uses pinning to sort sticky
    /// tasks above the rest.
    pub fn set_pinned(&self, task_id: &str, pinned: bool) -> Result<(), TasksError> {
        let conn = self.db.write()?;
        let affected = conn.execute(
            "UPDATE tasks SET is_pinned = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![pinned as i32, task_id],
        )?;
        if affected == 0 {
            return Err(TasksError::NotFound(task_id.to_string()));
        }
        Ok(())
    }

    /// Set or clear the linked-issue payload. The renderer stores
    /// provider + key (e.g. `linear:EMD-99`) as JSON so we don't
    /// need a typed Rust shape here.
    pub fn update_linked_issue(
        &self,
        task_id: &str,
        linked_issue: Option<String>,
    ) -> Result<(), TasksError> {
        let conn = self.db.write()?;
        let affected = conn.execute(
            "UPDATE tasks SET linked_issue = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![linked_issue, task_id],
        )?;
        if affected == 0 {
            return Err(TasksError::NotFound(task_id.to_string()));
        }
        Ok(())
    }

    fn project_path(&self, project_id: &str) -> Result<PathBuf, TasksError> {
        let conn = self.db.read()?;
        let row = conn
            .query_row(
                "SELECT path FROM projects WHERE id = ?",
                params![project_id],
                |r| r.get::<_, String>(0),
            )
            .optional()?;
        let path = row.ok_or_else(|| TasksError::ProjectNotFound(project_id.to_string()))?;
        let buf = PathBuf::from(&path);
        if !buf.is_dir() {
            return Err(TasksError::ProjectPathInvalid(path));
        }
        Ok(buf)
    }

    fn insert(
        &self,
        id: &str,
        project_id: &str,
        name: &str,
        path: &Path,
        task_branch: &str,
        source_branch: &TaskSourceBranch,
    ) -> Result<Task, TasksError> {
        let source_json = serde_json::to_string(source_branch)
            .map_err(|e| TasksError::MalformedSourceBranch(e.to_string()))?;
        let path_str = path.to_string_lossy().to_string();

        let conn = self.db.write()?;
        let mut stmt = conn.prepare(
            "INSERT INTO tasks (id, project_id, name, status, path, source_branch, task_branch) \
             VALUES (?, ?, ?, 'active', ?, ?, ?) \
             RETURNING id, project_id, name, status, path, source_branch, pty_id, created_at, updated_at",
        )?;
        let raw = stmt.query_row(
            params![id, project_id, name, &path_str, &source_json, task_branch],
            row_to_task,
        )?;
        decode_row_source_branch(raw)
    }
}

/// Branch the worktree will check out. Honors the renderer's explicit
/// choice (already pre-slugged and optionally prefixed) and falls back
/// to slugifying `name` when none was provided. Slashes are allowed
/// inside the slug so a renderer-supplied `feat/add-x` survives intact.
fn resolve_task_branch(explicit: &Option<String>, name: &str) -> Result<String, TasksError> {
    if let Some(b) = explicit.as_ref() {
        let trimmed = b.trim().trim_matches('/').to_string();
        if trimmed.is_empty() {
            return Err(TasksError::EmptyBranch);
        }
        return Ok(trimmed);
    }
    let slug = slug_branch(name);
    if slug.is_empty() {
        return Err(TasksError::EmptyBranch);
    }
    Ok(slug)
}

fn slug_branch(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else if c == '_' || c == '-' || c == '/' {
                c
            } else {
                '-'
            }
        })
        .collect();
    s.trim_matches('-').to_string()
}

/// True if a branch with this name already exists locally or as a
/// remote-tracking ref (e.g. `refs/remotes/origin/<name>`). Both
/// would cause `git worktree add -b <name>` to fail; surfacing the
/// collision up front gives the renderer a clean rename prompt.
fn branch_exists(project_root: &Path, branch: &str) -> Result<bool, TasksError> {
    use git2::{BranchType, Repository};

    let repo = match Repository::open(project_root) {
        Ok(r) => r,
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(false),
        Err(e) => return Err(TasksError::Git(e.into())),
    };
    if repo
        .find_branch(branch, BranchType::Local)
        .map(|_| true)
        .or_else(|e| match e.code() {
            git2::ErrorCode::NotFound => Ok(false),
            _ => Err(e),
        })
        .map_err(|e| TasksError::Git(e.into()))?
    {
        return Ok(true);
    }
    // Walk remotes looking for `<remote>/<branch>`. libgit2 stores
    // remote-tracking refs under `refs/remotes/<remote>/...`.
    for remote in repo
        .remotes()
        .map_err(|e| TasksError::Git(e.into()))?
        .iter()
        .flatten()
    {
        let qualified = format!("{remote}/{branch}");
        if repo.find_branch(&qualified, BranchType::Remote).is_ok() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Intermediate struct: every field except `source_branch` is loaded
/// directly, but `source_branch` requires JSON decoding before it can
/// satisfy `Task`. Two-phase keeps the rusqlite row mapper free of
/// `Result<Task, TasksError>` ambiguity.
struct RawTask {
    id: String,
    project_id: String,
    name: String,
    status: String,
    path: String,
    source_branch_json: Option<String>,
    pty_id: Option<String>,
    created_at: String,
    updated_at: String,
}

fn row_to_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawTask> {
    Ok(RawTask {
        id: row.get(0)?,
        project_id: row.get(1)?,
        name: row.get(2)?,
        status: row.get(3)?,
        path: row.get(4)?,
        source_branch_json: row.get(5)?,
        pty_id: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn decode_row_source_branch(raw: RawTask) -> Result<Task, TasksError> {
    let source_branch: TaskSourceBranch = match raw.source_branch_json.as_deref() {
        Some(s) => {
            serde_json::from_str(s).map_err(|e| TasksError::MalformedSourceBranch(e.to_string()))?
        }
        None => {
            // A NULL row would mean the schema invariant is broken — every
            // task should have a source_branch. Surface it explicitly
            // rather than silently defaulting to a stub.
            return Err(TasksError::MalformedSourceBranch(
                "source_branch is NULL".to_string(),
            ));
        }
    };
    let status = match raw.status.as_str() {
        "active" => TaskStatus::Active,
        "archived" => TaskStatus::Archived,
        other => {
            return Err(TasksError::MalformedSourceBranch(format!(
                "unknown task status: {other}"
            )))
        }
    };
    Ok(Task {
        id: raw.id,
        project_id: raw.project_id,
        name: raw.name,
        status,
        path: raw.path,
        source_branch,
        pty_id: raw.pty_id,
        created_at: raw.created_at,
        updated_at: raw.updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn init_project_repo() -> (TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        run(&path, &["init", "-q", "-b", "main"]);
        run(&path, &["config", "user.email", "test@example.com"]);
        run(&path, &["config", "user.name", "Test"]);
        run(&path, &["commit", "--allow-empty", "-m", "init"]);
        (dir, path)
    }

    fn run(repo: &Path, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn setup() -> (TempDir, TempDir, Arc<Db>, TasksService, String) {
        let (project_dir, project_path) = init_project_repo();
        let db_dir = tempfile::tempdir().unwrap();
        let db = Db::open(db_dir.path().join("test.db")).unwrap();
        let svc = TasksService::new(db.clone(), Arc::new(WorkspaceFsMutationLock::new()));
        // Seed a project row pointing at the temp repo.
        let project_id = Uuid::new_v4().to_string();
        db.write()
            .unwrap()
            .execute(
                "INSERT INTO projects (id, name, path) VALUES (?, ?, ?)",
                params![&project_id, "p", project_path.to_str().unwrap()],
            )
            .unwrap();
        (project_dir, db_dir, db, svc, project_id)
    }

    #[test]
    fn list_empty_returns_empty() {
        let (_p, _d, _db, svc, project_id) = setup();
        let tasks = svc.list(&project_id).unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn create_makes_worktree_and_db_row() {
        let (_p, _d, _db, svc, project_id) = setup();
        let task = svc
            .create(NewTaskInput {
                project_id: project_id.clone(),
                name: "Feature X".to_string(),
                source_branch: TaskSourceBranch::Local {
                    branch: "main".to_string(),
                },
                task_branch: None,
            })
            .unwrap();

        assert_eq!(task.project_id, project_id);
        assert_eq!(task.name, "Feature X");
        // Worktree path mirrors the branch — slug only, no UUID, no
        // `task/` prefix.
        assert!(task.path.ends_with(".emdash-worktrees/feature-x"));
        assert!(std::path::Path::new(&task.path).is_dir());
        assert!(matches!(task.status, TaskStatus::Active));
        assert!(task.pty_id.is_none());
        match task.source_branch {
            TaskSourceBranch::Local { ref branch } => assert_eq!(branch, "main"),
            _ => panic!("expected Local source_branch"),
        }

        let listed = svc.list(&project_id).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, task.id);
    }

    #[test]
    fn create_honors_explicit_task_branch_verbatim() {
        let (_p, _d, _db, svc, project_id) = setup();
        let task = svc
            .create(NewTaskInput {
                project_id: project_id.clone(),
                name: "Add search palette".to_string(),
                source_branch: TaskSourceBranch::Local {
                    branch: "main".to_string(),
                },
                task_branch: Some("feat/add-search".to_string()),
            })
            .unwrap();

        // Branch and worktree path mirror exactly — slashes preserved.
        assert!(task.path.ends_with(".emdash-worktrees/feat/add-search"));
        assert!(std::path::Path::new(&task.path).is_dir());
    }

    #[test]
    fn create_rejects_duplicate_branch() {
        let (_p, _d, _db, svc, project_id) = setup();
        svc.create(NewTaskInput {
            project_id: project_id.clone(),
            name: "first".to_string(),
            source_branch: TaskSourceBranch::Local {
                branch: "main".to_string(),
            },
            task_branch: Some("feat/clash".to_string()),
        })
        .unwrap();

        let err = svc
            .create(NewTaskInput {
                project_id: project_id.clone(),
                name: "second".to_string(),
                source_branch: TaskSourceBranch::Local {
                    branch: "main".to_string(),
                },
                task_branch: Some("feat/clash".to_string()),
            })
            .unwrap_err();
        assert!(
            matches!(err, TasksError::BranchAlreadyExists(ref b) if b == "feat/clash"),
            "expected BranchAlreadyExists, got {err:?}"
        );
    }

    #[test]
    fn delete_removes_worktree_and_row() {
        let (_p, _d, _db, svc, project_id) = setup();
        let task = svc
            .create(NewTaskInput {
                project_id: project_id.clone(),
                name: "x".to_string(),
                source_branch: TaskSourceBranch::Local {
                    branch: "main".to_string(),
                },
                task_branch: None,
            })
            .unwrap();
        let path = std::path::PathBuf::from(&task.path);
        assert!(path.is_dir());
        svc.delete(&task.id).unwrap();
        assert!(!path.exists(), "worktree dir should be gone");
        assert!(svc.list(&project_id).unwrap().is_empty());
    }

    #[test]
    fn delete_unknown_id_returns_not_found() {
        let (_p, _d, _db, svc, _project_id) = setup();
        let err = svc.delete("does-not-exist").unwrap_err();
        assert!(matches!(err, TasksError::NotFound(_)));
    }

    #[test]
    fn create_rejects_empty_name() {
        let (_p, _d, _db, svc, project_id) = setup();
        let err = svc
            .create(NewTaskInput {
                project_id: project_id.clone(),
                name: "   ".to_string(),
                source_branch: TaskSourceBranch::Local {
                    branch: "main".to_string(),
                },
                task_branch: None,
            })
            .unwrap_err();
        assert!(matches!(err, TasksError::EmptyName));
    }

    #[test]
    fn create_rejects_unknown_project() {
        let (_p, _d, _db, svc, _project_id) = setup();
        let err = svc
            .create(NewTaskInput {
                project_id: "ghost".to_string(),
                name: "x".to_string(),
                source_branch: TaskSourceBranch::Local {
                    branch: "main".to_string(),
                },
                task_branch: None,
            })
            .unwrap_err();
        assert!(matches!(err, TasksError::ProjectNotFound(_)));
    }

    #[test]
    fn list_filters_by_project() {
        let (_p, _d, db, svc, project_id) = setup();
        // Seed a second project (any dir path will do — we only list, never worktree-touch).
        let second_dir = tempfile::tempdir().unwrap();
        let second = Uuid::new_v4().to_string();
        db.write()
            .unwrap()
            .execute(
                "INSERT INTO projects (id, name, path) VALUES (?, ?, ?)",
                params![&second, "p2", second_dir.path().to_str().unwrap()],
            )
            .unwrap();

        svc.create(NewTaskInput {
            project_id: project_id.clone(),
            name: "a".to_string(),
            source_branch: TaskSourceBranch::Local {
                branch: "main".to_string(),
            },
            task_branch: None,
        })
        .unwrap();

        assert_eq!(svc.list(&project_id).unwrap().len(), 1);
        assert!(svc.list(&second).unwrap().is_empty());
    }

    #[test]
    fn slug_branch_strips_non_alnum() {
        assert_eq!(slug_branch("Hello World!"), "hello-world");
        assert_eq!(slug_branch("***"), "");
        assert_eq!(slug_branch("feat/already-prefixed"), "feat/already-prefixed");
    }

    #[test]
    fn resolve_task_branch_prefers_explicit() {
        let chosen = resolve_task_branch(&Some("feat/x".to_string()), "ignored").unwrap();
        assert_eq!(chosen, "feat/x");
    }

    #[test]
    fn resolve_task_branch_falls_back_to_slug() {
        let chosen = resolve_task_branch(&None, "Add Search").unwrap();
        assert_eq!(chosen, "add-search");
    }

    #[test]
    fn resolve_task_branch_rejects_empty_explicit_and_empty_slug() {
        assert!(matches!(
            resolve_task_branch(&Some("   ".to_string()), "x"),
            Err(TasksError::EmptyBranch)
        ));
        assert!(matches!(
            resolve_task_branch(&None, "***"),
            Err(TasksError::EmptyBranch)
        ));
    }
}
