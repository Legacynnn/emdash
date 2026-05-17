//! `WorkspacesService` — DB + filesystem CRUD for workspaces.
//!
//! Every write path is gated by `WorkspaceFsMutationLock` so concurrent
//! create/destroy calls on the same workspace id serialize, both for the
//! filesystem operation and the DB row update.
//!
//! Placement modes:
//! - `Worktree`: `git worktree add -b <branch> .emdash-worktrees/<branch> <source>`.
//! - `Local`: `git switch [-c] <branch>` inside the project root. At most one
//!   active local workspace per project (enforced by a partial-unique index).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::db::Db;

use super::fs_lock::WorkspaceFsMutationLock;
use super::model::{
    NewWorkspaceInput, Workspace, WorkspacePlacement, WorkspaceSourceBranch, WorkspaceStatus,
    WorkspacesError,
};
use super::worktree;

pub struct WorkspacesService {
    db: Arc<Db>,
    fs_lock: Arc<WorkspaceFsMutationLock>,
}

impl WorkspacesService {
    pub fn new(db: Arc<Db>, fs_lock: Arc<WorkspaceFsMutationLock>) -> Self {
        Self { db, fs_lock }
    }

    pub fn list(&self, project_id: &str) -> Result<Vec<Workspace>, WorkspacesError> {
        let conn = self.db.read()?;
        let mut stmt = conn.prepare(
            "SELECT id, project_id, name, status, placement, path, source_branch, created_at, updated_at \
             FROM workspaces \
             WHERE project_id = ? AND archived_at IS NULL \
             ORDER BY created_at DESC, id ASC",
        )?;
        let rows = stmt
            .query_map(params![project_id], row_to_workspace)?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(decode_row).collect()
    }

    pub fn get(&self, workspace_id: &str) -> Result<Option<Workspace>, WorkspacesError> {
        let conn = self.db.read()?;
        let row = conn
            .query_row(
                "SELECT id, project_id, name, status, placement, path, source_branch, created_at, updated_at \
                 FROM workspaces WHERE id = ?",
                params![workspace_id],
                row_to_workspace,
            )
            .optional()?;
        match row {
            Some(t) => decode_row(t).map(Some),
            None => Ok(None),
        }
    }

    /// Create a workspace. Worktree mode runs `git worktree add` and
    /// writes the DB row after the worktree exists. Local mode runs
    /// `git switch` in the project root after verifying the local-slot
    /// invariant and that the tree is clean.
    pub fn create(&self, input: NewWorkspaceInput) -> Result<Workspace, WorkspacesError> {
        if input.name.trim().is_empty() {
            return Err(WorkspacesError::EmptyName);
        }
        let project_path = self.project_path(&input.project_id)?;

        match input.placement {
            WorkspacePlacement::Worktree => self.create_worktree(input, &project_path),
            WorkspacePlacement::Local => self.create_local(input, &project_path),
        }
    }

    fn create_worktree(
        &self,
        input: NewWorkspaceInput,
        project_path: &Path,
    ) -> Result<Workspace, WorkspacesError> {
        let name = input.name.trim().to_string();
        let workspace_branch = resolve_workspace_branch(&input.workspace_branch, &name)?;
        if branch_exists(project_path, &workspace_branch)? {
            return Err(WorkspacesError::BranchAlreadyExists(workspace_branch));
        }

        let workspace_path = worktree::worktree_path(project_path, &workspace_branch);
        if workspace_path.exists() {
            return Err(WorkspacesError::BranchAlreadyExists(workspace_branch));
        }

        let workspace_id = Uuid::new_v4().to_string();
        let source = input.source_branch.checkout_target();

        let mutex = self.fs_lock.lock_for(&workspace_id);
        let _guard = mutex.lock();

        worktree::add(project_path, &workspace_path, &workspace_branch, &source)?;

        let row = self.insert(
            &workspace_id,
            &input.project_id,
            &name,
            WorkspacePlacement::Worktree,
            &workspace_path,
            &workspace_branch,
            &input.source_branch,
        );
        match row {
            Ok(workspace) => Ok(workspace),
            Err(insert_err) => {
                let _ = worktree::remove(project_path, &workspace_path);
                Err(insert_err)
            }
        }
    }

    fn create_local(
        &self,
        input: NewWorkspaceInput,
        project_path: &Path,
    ) -> Result<Workspace, WorkspacesError> {
        let name = input.name.trim().to_string();
        // One active local workspace per project. The partial-unique
        // index on `(project_id) WHERE placement='local' AND archived_at IS NULL`
        // is the ultimate enforcer; this check produces a friendlier error
        // than relying on the constraint violation.
        if let Some((existing_id, existing_name)) =
            self.active_local_workspace(&input.project_id)?
        {
            return Err(WorkspacesError::LocalSlotTaken {
                existing_workspace_id: existing_id,
                existing_workspace_name: existing_name,
            });
        }

        let dirty = dirty_files(project_path)?;
        if !dirty.is_empty() {
            return Err(WorkspacesError::DirtyTree {
                changed_files: dirty,
            });
        }

        let workspace_branch = if input.existing_branch {
            input
                .workspace_branch
                .as_ref()
                .map(|b| b.trim().trim_matches('/').to_string())
                .filter(|b| !b.is_empty())
                .ok_or(WorkspacesError::EmptyBranch)?
        } else {
            resolve_workspace_branch(&input.workspace_branch, &name)?
        };

        if input.existing_branch {
            if !branch_exists(project_path, &workspace_branch)? {
                return Err(WorkspacesError::BranchNotFound(workspace_branch));
            }
            git_switch(project_path, &workspace_branch)?;
        } else {
            if branch_exists(project_path, &workspace_branch)? {
                return Err(WorkspacesError::BranchAlreadyExists(workspace_branch));
            }
            git_switch_create(project_path, &workspace_branch, &input.source_branch.checkout_target())?;
        }

        let workspace_id = Uuid::new_v4().to_string();
        let mutex = self.fs_lock.lock_for(&workspace_id);
        let _guard = mutex.lock();

        self.insert(
            &workspace_id,
            &input.project_id,
            &name,
            WorkspacePlacement::Local,
            project_path,
            &workspace_branch,
            &input.source_branch,
        )
    }

    pub fn delete(&self, workspace_id: &str) -> Result<(), WorkspacesError> {
        let workspace = self
            .get(workspace_id)?
            .ok_or_else(|| WorkspacesError::NotFound(workspace_id.to_string()))?;
        let project_path = self.project_path(&workspace.project_id)?;

        let mutex = self.fs_lock.lock_for(workspace_id);
        let _guard = mutex.lock();

        match workspace.placement {
            WorkspacePlacement::Worktree => {
                // FS first; proceed to DB cleanup even if the worktree dir
                // is already gone so the user can recover from a half-deleted
                // state.
                let workspace_path = PathBuf::from(&workspace.path);
                let _ = worktree::remove(&project_path, &workspace_path);
            }
            WorkspacePlacement::Local => {
                // Local mode leaves the branch and working tree as-is.
                // Removing emdash's record doesn't destroy the user's
                // in-place work.
            }
        }

        let conn = self.db.write()?;
        let affected = conn.execute("DELETE FROM workspaces WHERE id = ?", params![workspace_id])?;
        if affected == 0 {
            return Err(WorkspacesError::NotFound(workspace_id.to_string()));
        }
        Ok(())
    }

    /// Update the user-facing `name` of a workspace. Does not touch the
    /// underlying git branch; the renderer keeps display name and
    /// branch name as separate concerns.
    pub fn rename(&self, workspace_id: &str, name: &str) -> Result<Workspace, WorkspacesError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(WorkspacesError::EmptyName);
        }
        let conn = self.db.write()?;
        let affected = conn.execute(
            "UPDATE workspaces SET name = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![trimmed, workspace_id],
        )?;
        if affected == 0 {
            return Err(WorkspacesError::NotFound(workspace_id.to_string()));
        }
        drop(conn);
        self.get(workspace_id)?
            .ok_or_else(|| WorkspacesError::NotFound(workspace_id.to_string()))
    }

    pub fn archive(&self, workspace_id: &str) -> Result<Workspace, WorkspacesError> {
        let conn = self.db.write()?;
        let affected = conn.execute(
            "UPDATE workspaces \
             SET status = 'archived', archived_at = CURRENT_TIMESTAMP, \
                 status_changed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
             WHERE id = ?1",
            params![workspace_id],
        )?;
        if affected == 0 {
            return Err(WorkspacesError::NotFound(workspace_id.to_string()));
        }
        drop(conn);
        self.get(workspace_id)?
            .ok_or_else(|| WorkspacesError::NotFound(workspace_id.to_string()))
    }

    pub fn restore(&self, workspace_id: &str) -> Result<Workspace, WorkspacesError> {
        let conn = self.db.write()?;
        let affected = conn.execute(
            "UPDATE workspaces \
             SET status = 'active', archived_at = NULL, \
                 status_changed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
             WHERE id = ?1",
            params![workspace_id],
        )?;
        if affected == 0 {
            return Err(WorkspacesError::NotFound(workspace_id.to_string()));
        }
        drop(conn);
        self.get(workspace_id)?
            .ok_or_else(|| WorkspacesError::NotFound(workspace_id.to_string()))
    }

    pub fn set_pinned(&self, workspace_id: &str, pinned: bool) -> Result<(), WorkspacesError> {
        let conn = self.db.write()?;
        let affected = conn.execute(
            "UPDATE workspaces SET is_pinned = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![pinned as i32, workspace_id],
        )?;
        if affected == 0 {
            return Err(WorkspacesError::NotFound(workspace_id.to_string()));
        }
        Ok(())
    }

    pub fn update_linked_issue(
        &self,
        workspace_id: &str,
        linked_issue: Option<String>,
    ) -> Result<(), WorkspacesError> {
        let conn = self.db.write()?;
        let affected = conn.execute(
            "UPDATE workspaces SET linked_issue = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![linked_issue, workspace_id],
        )?;
        if affected == 0 {
            return Err(WorkspacesError::NotFound(workspace_id.to_string()));
        }
        Ok(())
    }

    fn project_path(&self, project_id: &str) -> Result<PathBuf, WorkspacesError> {
        let conn = self.db.read()?;
        let row = conn
            .query_row(
                "SELECT path FROM projects WHERE id = ?",
                params![project_id],
                |r| r.get::<_, String>(0),
            )
            .optional()?;
        let path = row.ok_or_else(|| WorkspacesError::ProjectNotFound(project_id.to_string()))?;
        let buf = PathBuf::from(&path);
        if !buf.is_dir() {
            return Err(WorkspacesError::ProjectPathInvalid(path));
        }
        Ok(buf)
    }

    fn active_local_workspace(
        &self,
        project_id: &str,
    ) -> Result<Option<(String, String)>, WorkspacesError> {
        let conn = self.db.read()?;
        let row = conn
            .query_row(
                "SELECT id, name FROM workspaces \
                 WHERE project_id = ?1 AND placement = 'local' AND archived_at IS NULL \
                 LIMIT 1",
                params![project_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?;
        Ok(row)
    }

    fn insert(
        &self,
        id: &str,
        project_id: &str,
        name: &str,
        placement: WorkspacePlacement,
        path: &Path,
        workspace_branch: &str,
        source_branch: &WorkspaceSourceBranch,
    ) -> Result<Workspace, WorkspacesError> {
        let source_json = serde_json::to_string(source_branch)
            .map_err(|e| WorkspacesError::MalformedSourceBranch(e.to_string()))?;
        let path_str = path.to_string_lossy().to_string();
        let placement_str = placement_to_str(placement);

        let conn = self.db.write()?;
        let mut stmt = conn.prepare(
            "INSERT INTO workspaces (id, project_id, name, status, placement, path, source_branch, workspace_branch) \
             VALUES (?, ?, ?, 'active', ?, ?, ?, ?) \
             RETURNING id, project_id, name, status, placement, path, source_branch, created_at, updated_at",
        )?;
        let raw = stmt.query_row(
            params![id, project_id, name, placement_str, &path_str, &source_json, workspace_branch],
            row_to_workspace,
        )?;
        decode_row(raw)
    }
}

fn placement_to_str(p: WorkspacePlacement) -> &'static str {
    match p {
        WorkspacePlacement::Worktree => "worktree",
        WorkspacePlacement::Local => "local",
    }
}

/// Branch the workspace will check out. Honors the renderer's explicit
/// choice and falls back to slugifying `name` when none was provided.
/// Slashes are allowed inside the slug so a renderer-supplied
/// `feat/add-x` survives intact.
fn resolve_workspace_branch(
    explicit: &Option<String>,
    name: &str,
) -> Result<String, WorkspacesError> {
    if let Some(b) = explicit.as_ref() {
        let trimmed = b.trim().trim_matches('/').to_string();
        if trimmed.is_empty() {
            return Err(WorkspacesError::EmptyBranch);
        }
        return Ok(trimmed);
    }
    let slug = slug_branch(name);
    if slug.is_empty() {
        return Err(WorkspacesError::EmptyBranch);
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
/// remote-tracking ref.
fn branch_exists(project_root: &Path, branch: &str) -> Result<bool, WorkspacesError> {
    use git2::{BranchType, Repository};

    let repo = match Repository::open(project_root) {
        Ok(r) => r,
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(false),
        Err(e) => return Err(WorkspacesError::Git(e.into())),
    };
    if repo
        .find_branch(branch, BranchType::Local)
        .map(|_| true)
        .or_else(|e| match e.code() {
            git2::ErrorCode::NotFound => Ok(false),
            _ => Err(e),
        })
        .map_err(|e| WorkspacesError::Git(e.into()))?
    {
        return Ok(true);
    }
    for remote in repo
        .remotes()
        .map_err(|e| WorkspacesError::Git(e.into()))?
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

/// `git status --porcelain` filenames, or empty if the tree is clean.
fn dirty_files(project_root: &Path) -> Result<Vec<String>, WorkspacesError> {
    let output = Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(project_root)
        .output()
        .map_err(|e| WorkspacesError::SwitchFailed(format!("spawn git status: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(WorkspacesError::SwitchFailed(format!(
            "git status failed: {stderr}"
        )));
    }
    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.get(3..).map(|s| s.to_string()))
        .collect();
    Ok(files)
}

fn git_switch(project_root: &Path, branch: &str) -> Result<(), WorkspacesError> {
    let output = Command::new("git")
        .arg("switch")
        .arg(branch)
        .current_dir(project_root)
        .output()
        .map_err(|e| WorkspacesError::SwitchFailed(format!("spawn git: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(WorkspacesError::SwitchFailed(format!(
            "git switch failed: {stderr}"
        )));
    }
    Ok(())
}

fn git_switch_create(
    project_root: &Path,
    branch: &str,
    source: &str,
) -> Result<(), WorkspacesError> {
    let output = Command::new("git")
        .arg("switch")
        .arg("-c")
        .arg(branch)
        .arg(source)
        .current_dir(project_root)
        .output()
        .map_err(|e| WorkspacesError::SwitchFailed(format!("spawn git: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(WorkspacesError::SwitchFailed(format!(
            "git switch -c failed: {stderr}"
        )));
    }
    Ok(())
}

struct RawWorkspace {
    id: String,
    project_id: String,
    name: String,
    status: String,
    placement: String,
    path: String,
    source_branch_json: Option<String>,
    created_at: String,
    updated_at: String,
}

fn row_to_workspace(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawWorkspace> {
    Ok(RawWorkspace {
        id: row.get(0)?,
        project_id: row.get(1)?,
        name: row.get(2)?,
        status: row.get(3)?,
        placement: row.get(4)?,
        path: row.get(5)?,
        source_branch_json: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn decode_row(raw: RawWorkspace) -> Result<Workspace, WorkspacesError> {
    let source_branch: WorkspaceSourceBranch = match raw.source_branch_json.as_deref() {
        Some(s) => serde_json::from_str(s)
            .map_err(|e| WorkspacesError::MalformedSourceBranch(e.to_string()))?,
        None => {
            return Err(WorkspacesError::MalformedSourceBranch(
                "source_branch is NULL".to_string(),
            ));
        }
    };
    let status = match raw.status.as_str() {
        "active" => WorkspaceStatus::Active,
        "archived" => WorkspaceStatus::Archived,
        other => {
            return Err(WorkspacesError::MalformedSourceBranch(format!(
                "unknown workspace status: {other}"
            )))
        }
    };
    let placement = match raw.placement.as_str() {
        "worktree" => WorkspacePlacement::Worktree,
        "local" => WorkspacePlacement::Local,
        other => {
            return Err(WorkspacesError::MalformedSourceBranch(format!(
                "unknown workspace placement: {other}"
            )))
        }
    };
    Ok(Workspace {
        id: raw.id,
        project_id: raw.project_id,
        name: raw.name,
        status,
        placement,
        path: raw.path,
        source_branch,
        created_at: raw.created_at,
        updated_at: raw.updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn setup() -> (TempDir, TempDir, Arc<Db>, WorkspacesService, String) {
        let (project_dir, project_path) = init_project_repo();
        let db_dir = tempfile::tempdir().unwrap();
        let db = Db::open(db_dir.path().join("test.db")).unwrap();
        let svc = WorkspacesService::new(db.clone(), Arc::new(WorkspaceFsMutationLock::new()));
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

    fn worktree_input(project_id: &str, name: &str, branch: Option<&str>) -> NewWorkspaceInput {
        NewWorkspaceInput {
            project_id: project_id.to_string(),
            name: name.to_string(),
            source_branch: WorkspaceSourceBranch::Local {
                branch: "main".to_string(),
            },
            workspace_branch: branch.map(String::from),
            placement: WorkspacePlacement::Worktree,
            existing_branch: false,
        }
    }

    fn local_new_input(project_id: &str, name: &str) -> NewWorkspaceInput {
        NewWorkspaceInput {
            project_id: project_id.to_string(),
            name: name.to_string(),
            source_branch: WorkspaceSourceBranch::Local {
                branch: "main".to_string(),
            },
            workspace_branch: None,
            placement: WorkspacePlacement::Local,
            existing_branch: false,
        }
    }

    #[test]
    fn list_empty_returns_empty() {
        let (_p, _d, _db, svc, project_id) = setup();
        let workspaces = svc.list(&project_id).unwrap();
        assert!(workspaces.is_empty());
    }

    #[test]
    fn create_worktree_makes_dir_and_db_row() {
        let (_p, _d, _db, svc, project_id) = setup();
        let workspace = svc
            .create(worktree_input(&project_id, "Feature X", None))
            .unwrap();
        assert_eq!(workspace.project_id, project_id);
        assert_eq!(workspace.name, "Feature X");
        assert_eq!(workspace.placement, WorkspacePlacement::Worktree);
        assert!(workspace.path.ends_with(".emdash-worktrees/feature-x"));
        assert!(std::path::Path::new(&workspace.path).is_dir());
        assert!(matches!(workspace.status, WorkspaceStatus::Active));
    }

    #[test]
    fn create_worktree_honors_explicit_branch() {
        let (_p, _d, _db, svc, project_id) = setup();
        let workspace = svc
            .create(worktree_input(
                &project_id,
                "Add search palette",
                Some("feat/add-search"),
            ))
            .unwrap();
        assert!(workspace.path.ends_with(".emdash-worktrees/feat/add-search"));
        assert!(std::path::Path::new(&workspace.path).is_dir());
    }

    #[test]
    fn create_worktree_rejects_duplicate_branch() {
        let (_p, _d, _db, svc, project_id) = setup();
        svc.create(worktree_input(&project_id, "first", Some("feat/clash")))
            .unwrap();
        let err = svc
            .create(worktree_input(&project_id, "second", Some("feat/clash")))
            .unwrap_err();
        assert!(matches!(err, WorkspacesError::BranchAlreadyExists(ref b) if b == "feat/clash"));
    }

    #[test]
    fn delete_worktree_removes_dir_and_row() {
        let (_p, _d, _db, svc, project_id) = setup();
        let workspace = svc.create(worktree_input(&project_id, "x", None)).unwrap();
        let path = PathBuf::from(&workspace.path);
        assert!(path.is_dir());
        svc.delete(&workspace.id).unwrap();
        assert!(!path.exists(), "worktree dir should be gone");
        assert!(svc.list(&project_id).unwrap().is_empty());
    }

    #[test]
    fn create_rejects_empty_name() {
        let (_p, _d, _db, svc, project_id) = setup();
        let mut input = worktree_input(&project_id, "   ", None);
        input.name = "   ".to_string();
        let err = svc.create(input).unwrap_err();
        assert!(matches!(err, WorkspacesError::EmptyName));
    }

    #[test]
    fn create_rejects_unknown_project() {
        let (_p, _d, _db, svc, _project_id) = setup();
        let err = svc
            .create(worktree_input("ghost", "x", None))
            .unwrap_err();
        assert!(matches!(err, WorkspacesError::ProjectNotFound(_)));
    }

    #[test]
    fn create_local_new_branch_switches_in_place() {
        let (_p, _d, _db, svc, project_id) = setup();
        let workspace = svc
            .create(local_new_input(&project_id, "in-place feature"))
            .unwrap();
        assert_eq!(workspace.placement, WorkspacePlacement::Local);
        // Local path = project path
        let project_path = svc.project_path(&project_id).unwrap();
        assert_eq!(workspace.path, project_path.to_string_lossy());
    }

    #[test]
    fn create_local_rejects_when_slot_taken() {
        let (_p, _d, _db, svc, project_id) = setup();
        svc.create(local_new_input(&project_id, "first")).unwrap();
        let err = svc
            .create(local_new_input(&project_id, "second"))
            .unwrap_err();
        assert!(matches!(err, WorkspacesError::LocalSlotTaken { .. }));
    }

    #[test]
    fn create_local_rejects_dirty_tree() {
        let (_p, _d, _db, svc, project_id) = setup();
        let project_path = svc.project_path(&project_id).unwrap();
        std::fs::write(project_path.join("dirty.txt"), "untracked").unwrap();
        let err = svc
            .create(local_new_input(&project_id, "x"))
            .unwrap_err();
        assert!(matches!(err, WorkspacesError::DirtyTree { .. }));
    }

    #[test]
    fn create_local_existing_branch_requires_existing() {
        let (_p, _d, _db, svc, project_id) = setup();
        let mut input = local_new_input(&project_id, "x");
        input.existing_branch = true;
        input.workspace_branch = Some("missing-branch".to_string());
        let err = svc.create(input).unwrap_err();
        assert!(matches!(err, WorkspacesError::BranchNotFound(_)));
    }

    #[test]
    fn delete_local_leaves_branch_intact() {
        let (_p, _d, _db, svc, project_id) = setup();
        let project_path = svc.project_path(&project_id).unwrap();
        let workspace = svc
            .create(local_new_input(&project_id, "in-place"))
            .unwrap();
        // Branch should exist
        let branch_name = "in-place".to_string();
        assert!(branch_exists(&project_path, &branch_name).unwrap());
        svc.delete(&workspace.id).unwrap();
        assert!(svc.list(&project_id).unwrap().is_empty());
        // Branch survives
        assert!(branch_exists(&project_path, &branch_name).unwrap());
    }

    #[test]
    fn list_filters_by_project() {
        let (_p, _d, db, svc, project_id) = setup();
        let second_dir = tempfile::tempdir().unwrap();
        let second = Uuid::new_v4().to_string();
        db.write()
            .unwrap()
            .execute(
                "INSERT INTO projects (id, name, path) VALUES (?, ?, ?)",
                params![&second, "p2", second_dir.path().to_str().unwrap()],
            )
            .unwrap();
        svc.create(worktree_input(&project_id, "a", None)).unwrap();
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
    fn resolve_workspace_branch_prefers_explicit() {
        let chosen = resolve_workspace_branch(&Some("feat/x".to_string()), "ignored").unwrap();
        assert_eq!(chosen, "feat/x");
    }

    #[test]
    fn resolve_workspace_branch_falls_back_to_slug() {
        let chosen = resolve_workspace_branch(&None, "Add Search").unwrap();
        assert_eq!(chosen, "add-search");
    }

    #[test]
    fn resolve_workspace_branch_rejects_empty() {
        assert!(matches!(
            resolve_workspace_branch(&Some("   ".to_string()), "x"),
            Err(WorkspacesError::EmptyBranch)
        ));
        assert!(matches!(
            resolve_workspace_branch(&None, "***"),
            Err(WorkspacesError::EmptyBranch)
        ));
    }
}
