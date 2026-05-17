//! `TerminalsService` — CRUD over the `terminals` table.

use std::sync::Arc;

use rusqlite::params;
use uuid::Uuid;

use crate::db::Db;

use super::model::{NewTerminalInput, Terminal, TerminalsError};

pub struct TerminalsService {
    db: Arc<Db>,
}

impl TerminalsService {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub fn list_for_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<Terminal>, TerminalsError> {
        let conn = self.db.read()?;
        let mut stmt = conn.prepare(
            "SELECT id, project_id, workspace_id, name, ssh, created_at, updated_at \
             FROM terminals WHERE workspace_id = ?1 \
             ORDER BY created_at ASC, id ASC",
        )?;
        let rows = stmt
            .query_map(params![workspace_id], row_to_terminal)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get(&self, id: &str) -> Result<Option<Terminal>, TerminalsError> {
        let conn = self.db.read()?;
        let mut stmt = conn.prepare(
            "SELECT id, project_id, workspace_id, name, ssh, created_at, updated_at \
             FROM terminals WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], row_to_terminal)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    pub fn create(&self, input: NewTerminalInput) -> Result<Terminal, TerminalsError> {
        let trimmed = input.name.trim();
        if trimmed.is_empty() {
            return Err(TerminalsError::EmptyName);
        }
        let workspace_exists: bool = self
            .db
            .read()?
            .query_row(
                "SELECT 1 FROM workspaces WHERE id = ?1",
                params![input.workspace_id],
                |_| Ok(true),
            )
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(false),
                _ => Err(e),
            })?;
        if !workspace_exists {
            return Err(TerminalsError::WorkspaceNotFound(input.workspace_id));
        }

        let id = input.id.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
        let conn = self.db.write()?;
        conn.execute(
            "INSERT INTO terminals (id, project_id, workspace_id, name, ssh) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id,
                input.project_id,
                input.workspace_id,
                trimmed,
                input.ssh.unwrap_or(false) as i32,
            ],
        )?;
        drop(conn);

        self.get(&id)?
            .ok_or_else(|| TerminalsError::NotFound(id))
    }

    pub fn rename(&self, id: &str, name: &str) -> Result<(), TerminalsError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(TerminalsError::EmptyName);
        }
        let conn = self.db.write()?;
        let affected = conn.execute(
            "UPDATE terminals SET name = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![trimmed, id],
        )?;
        if affected == 0 {
            return Err(TerminalsError::NotFound(id.into()));
        }
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), TerminalsError> {
        let conn = self.db.write()?;
        let affected =
            conn.execute("DELETE FROM terminals WHERE id = ?1", params![id])?;
        if affected == 0 {
            return Err(TerminalsError::NotFound(id.into()));
        }
        Ok(())
    }
}

fn row_to_terminal(row: &rusqlite::Row<'_>) -> rusqlite::Result<Terminal> {
    let ssh_int: i32 = row.get(4)?;
    Ok(Terminal {
        id: row.get(0)?,
        project_id: row.get(1)?,
        workspace_id: row.get(2)?,
        name: row.get(3)?,
        ssh: ssh_int != 0,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}
