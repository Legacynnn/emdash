//! `ProjectsService` — CRUD over the `projects` table.
//!
//! All write paths go through `db.write()` (the single-writer pool from
//! the EMD-6 foundation). Reads use `db.read()` so concurrent listings
//! don't serialize behind writers.

use std::path::Path;
use std::sync::Arc;

use rusqlite::params;
use uuid::Uuid;

use crate::db::Db;

use super::model::{Project, ProjectsError};

pub struct ProjectsService {
    db: Arc<Db>,
}

impl ProjectsService {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// List all projects, newest first by `created_at`. Stable enough to
    /// drive the renderer list without an explicit cursor.
    pub fn get(&self, id: &str) -> Result<Option<Project>, ProjectsError> {
        let conn = self.db.read()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, path, created_at, updated_at FROM projects WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(rusqlite::params![id], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    pub fn list(&self) -> Result<Vec<Project>, ProjectsError> {
        let conn = self.db.read()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, path, created_at, updated_at \
             FROM projects \
             ORDER BY created_at DESC, id ASC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Add a project pointing at `path`. The path must be absolute and
    /// must already exist as a directory on disk. UUID-v4 id; basename
    /// of the path becomes the initial name (the renderer can rename
    /// later via a future `projects.update` command).
    pub fn add(&self, path: &str) -> Result<Project, ProjectsError> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err(ProjectsError::EmptyPath);
        }
        let path_buf = Path::new(trimmed);
        if !path_buf.is_absolute() {
            return Err(ProjectsError::NotAbsolute(trimmed.to_string()));
        }
        if !path_buf.exists() {
            return Err(ProjectsError::PathMissing(trimmed.to_string()));
        }
        if !path_buf.is_dir() {
            return Err(ProjectsError::PathNotDirectory(trimmed.to_string()));
        }

        let name = derive_name(path_buf, trimmed);
        let id = Uuid::new_v4().to_string();

        let conn = self.db.write()?;
        let mut stmt = conn.prepare(
            "INSERT INTO projects (id, name, path) VALUES (?, ?, ?) \
             RETURNING id, name, path, created_at, updated_at",
        )?;
        let project = stmt
            .query_row(params![&id, &name, trimmed], |row| {
                Ok(Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(err, _)
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    ProjectsError::DuplicatePath(trimmed.to_string())
                }
                other => ProjectsError::Sqlite(other),
            })?;
        Ok(project)
    }

    /// Remove a project by id. ON DELETE CASCADE on dependent tables
    /// (`tasks`, `conversations`, etc.) handles the rest.
    pub fn remove(&self, id: &str) -> Result<(), ProjectsError> {
        let conn = self.db.write()?;
        let affected = conn.execute("DELETE FROM projects WHERE id = ?", params![id])?;
        if affected == 0 {
            return Err(ProjectsError::NotFound(id.to_string()));
        }
        Ok(())
    }
}

fn derive_name(path: &Path, fallback: &str) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| fallback.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, Arc<Db>, ProjectsService) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.db");
        let db = Db::open(&path).expect("open db");
        let svc = ProjectsService::new(db.clone());
        (dir, db, svc)
    }

    fn temp_dir_path() -> (TempDir, String) {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().to_str().unwrap().to_string();
        (dir, p)
    }

    #[test]
    fn list_empty_returns_empty_vec() {
        let (_dir, _db, svc) = setup();
        assert!(svc.list().unwrap().is_empty());
    }

    #[test]
    fn add_creates_row_with_basename_as_name() {
        let (_dir, _db, svc) = setup();
        let (_project_dir, path) = temp_dir_path();

        let project = svc.add(&path).expect("add");
        assert_eq!(project.path, path);
        // Tempdir basename: the unique-suffixed dir tempfile creates.
        let expected_name = Path::new(&path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert_eq!(project.name, expected_name);
        assert!(!project.id.is_empty());
        assert!(!project.created_at.is_empty());

        let listed = svc.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, project.id);
    }

    #[test]
    fn add_rejects_empty_path() {
        let (_dir, _db, svc) = setup();
        let err = svc.add("   ").unwrap_err();
        assert!(matches!(err, ProjectsError::EmptyPath));
    }

    #[test]
    fn add_rejects_relative_path() {
        let (_dir, _db, svc) = setup();
        let err = svc.add("./relative").unwrap_err();
        assert!(matches!(err, ProjectsError::NotAbsolute(_)));
    }

    #[test]
    fn add_rejects_missing_path() {
        let (_dir, _db, svc) = setup();
        let err = svc
            .add("/this/path/should/not/exist/anywhere/q9w8e7r6")
            .unwrap_err();
        assert!(matches!(err, ProjectsError::PathMissing(_)));
    }

    #[test]
    fn add_rejects_file_not_dir() {
        let (_dir, _db, svc) = setup();
        let file = tempfile::NamedTempFile::new().unwrap();
        let err = svc.add(file.path().to_str().unwrap()).unwrap_err();
        assert!(matches!(err, ProjectsError::PathNotDirectory(_)));
    }

    #[test]
    fn add_rejects_duplicate_path() {
        let (_dir, _db, svc) = setup();
        let (_project_dir, path) = temp_dir_path();
        svc.add(&path).expect("first add");
        let err = svc.add(&path).unwrap_err();
        assert!(matches!(err, ProjectsError::DuplicatePath(_)));
    }

    #[test]
    fn remove_deletes_row() {
        let (_dir, _db, svc) = setup();
        let (_project_dir, path) = temp_dir_path();
        let project = svc.add(&path).unwrap();
        svc.remove(&project.id).unwrap();
        assert!(svc.list().unwrap().is_empty());
    }

    #[test]
    fn remove_unknown_id_returns_not_found() {
        let (_dir, _db, svc) = setup();
        let err = svc.remove("nope").unwrap_err();
        assert!(matches!(err, ProjectsError::NotFound(_)));
    }

    #[test]
    fn list_orders_newest_first() {
        let (_dir, _db, svc) = setup();
        let (_a, path_a) = temp_dir_path();
        let (_b, path_b) = temp_dir_path();
        // SQLite's CURRENT_TIMESTAMP has 1-second resolution; sleep so
        // the second insert is strictly newer.
        let first = svc.add(&path_a).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let second = svc.add(&path_b).unwrap();

        let listed = svc.list().unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, second.id, "newest first");
        assert_eq!(listed[1].id, first.id);
    }
}
