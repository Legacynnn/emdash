//! `editor/` port from Electron (ADR-0012). Tiny CRUD over the
//! `editor_buffers` table — Monaco unsaved-state preservation.
//! Tauri-runtime-free; the glue in `commands::editor_buffers` only
//! threads `tauri::State<Arc<Db>>` through.

use std::sync::Arc;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

use crate::db::{Db, DbError};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct EditorBuffer {
    pub id: String,
    pub project_id: String,
    pub workspace_id: String,
    pub file_path: String,
    pub content: String,
    /// Milliseconds-since-epoch when the row was last written, serialized
    /// as a string at the wire boundary because specta refuses to emit
    /// i64 (BigInt-style types lose precision through `JSON.parse`).
    pub updated_at_ms: String,
}

#[derive(Debug, Error)]
pub enum EditorBuffersError {
    #[error("db error: {0}")]
    Db(#[from] DbError),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn buffer_id(project_id: &str, workspace_id: &str, file_path: &str) -> String {
    // Deterministic per (project, workspace, file) so save_buffer is
    // an upsert: callers don't have to look up the row first.
    format!("{project_id}|{workspace_id}|{file_path}")
}

pub fn save(
    db: &Arc<Db>,
    project_id: &str,
    workspace_id: &str,
    file_path: &str,
    content: &str,
) -> Result<EditorBuffer, EditorBuffersError> {
    let id = buffer_id(project_id, workspace_id, file_path);
    let updated_at = now_ms();
    let conn = db.write()?;
    conn.execute(
        "INSERT INTO editor_buffers (id, project_id, workspace_id, file_path, content, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
        params![&id, project_id, workspace_id, file_path, content, updated_at],
    )?;
    Ok(EditorBuffer {
        id,
        project_id: project_id.to_string(),
        workspace_id: workspace_id.to_string(),
        file_path: file_path.to_string(),
        content: content.to_string(),
        updated_at_ms: updated_at.to_string(),
    })
}

pub fn clear(
    db: &Arc<Db>,
    project_id: &str,
    workspace_id: &str,
    file_path: &str,
) -> Result<(), EditorBuffersError> {
    let id = buffer_id(project_id, workspace_id, file_path);
    let conn = db.write()?;
    conn.execute("DELETE FROM editor_buffers WHERE id = ?", params![&id])?;
    Ok(())
}

pub fn list(
    db: &Arc<Db>,
    project_id: &str,
    workspace_id: &str,
) -> Result<Vec<EditorBuffer>, EditorBuffersError> {
    let conn = db.read()?;
    let mut stmt = conn.prepare(
        "SELECT id, project_id, workspace_id, file_path, content, updated_at \
         FROM editor_buffers \
         WHERE project_id = ? AND workspace_id = ? \
         ORDER BY updated_at DESC",
    )?;
    let rows = stmt
        .query_map(params![project_id, workspace_id], |row| {
            let updated_at: i64 = row.get(5)?;
            Ok(EditorBuffer {
                id: row.get(0)?,
                project_id: row.get(1)?,
                workspace_id: row.get(2)?,
                file_path: row.get(3)?,
                content: row.get(4)?,
                updated_at_ms: updated_at.to_string(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn open_temp_db() -> (TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path().join("test.db")).unwrap();
        (dir, db)
    }

    fn seed_project(db: &Arc<Db>, id: &str) {
        db.write()
            .unwrap()
            .execute(
                "INSERT INTO projects (id, name, path) VALUES (?, ?, ?)",
                params![id, "p", format!("/tmp/{id}")],
            )
            .unwrap();
    }

    #[test]
    fn save_inserts_then_updates() {
        let (_d, db) = open_temp_db();
        seed_project(&db, "p1");
        let a = save(&db, "p1", "ws1", "a.txt", "hello").unwrap();
        let b = save(&db, "p1", "ws1", "a.txt", "world").unwrap();
        assert_eq!(a.id, b.id, "upsert keeps the same id");
        assert_eq!(b.content, "world");
    }

    #[test]
    fn list_filters_by_workspace() {
        let (_d, db) = open_temp_db();
        seed_project(&db, "p1");
        save(&db, "p1", "ws1", "a.txt", "x").unwrap();
        save(&db, "p1", "ws2", "b.txt", "y").unwrap();
        let ws1 = list(&db, "p1", "ws1").unwrap();
        assert_eq!(ws1.len(), 1);
        assert_eq!(ws1[0].file_path, "a.txt");
    }

    #[test]
    fn clear_removes_the_row() {
        let (_d, db) = open_temp_db();
        seed_project(&db, "p1");
        save(&db, "p1", "ws1", "a.txt", "x").unwrap();
        clear(&db, "p1", "ws1", "a.txt").unwrap();
        assert!(list(&db, "p1", "ws1").unwrap().is_empty());
    }
}
