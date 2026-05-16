//! `ConversationsService` — CRUD over the `conversations` table.

use std::sync::Arc;

use rusqlite::params;
use uuid::Uuid;

use crate::db::Db;

use super::model::{Conversation, ConversationsError, NewConversationInput};

pub struct ConversationsService {
    db: Arc<Db>,
}

impl ConversationsService {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// List conversations belonging to `task_id`. Newest-interacted
    /// first so the renderer's tab order matches recency.
    pub fn list_for_task(&self, task_id: &str) -> Result<Vec<Conversation>, ConversationsError> {
        let conn = self.db.read()?;
        let mut stmt = conn.prepare(
            "SELECT id, project_id, task_id, title, provider, config, \
                    COALESCE(is_initial_conversation, 0), last_interacted_at, \
                    created_at, updated_at \
             FROM conversations \
             WHERE task_id = ?1 \
             ORDER BY COALESCE(last_interacted_at, created_at) DESC, id ASC",
        )?;
        let rows = stmt
            .query_map(params![task_id], row_to_conversation)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get(&self, id: &str) -> Result<Option<Conversation>, ConversationsError> {
        let conn = self.db.read()?;
        let mut stmt = conn.prepare(
            "SELECT id, project_id, task_id, title, provider, config, \
                    COALESCE(is_initial_conversation, 0), last_interacted_at, \
                    created_at, updated_at \
             FROM conversations \
             WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], row_to_conversation)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    pub fn create(
        &self,
        input: NewConversationInput,
    ) -> Result<Conversation, ConversationsError> {
        let trimmed = input.title.trim();
        if trimmed.is_empty() {
            return Err(ConversationsError::EmptyTitle);
        }
        let id = Uuid::new_v4().to_string();
        let task_exists: bool = self
            .db
            .read()?
            .query_row(
                "SELECT 1 FROM tasks WHERE id = ?1",
                params![input.task_id],
                |_| Ok(true),
            )
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(false),
                _ => Err(e),
            })?;
        if !task_exists {
            return Err(ConversationsError::TaskNotFound(input.task_id));
        }

        let now_iso = chrono::Utc::now().to_rfc3339();
        let is_initial = input.is_initial_conversation.unwrap_or(false);
        let conn = self.db.write()?;
        conn.execute(
            "INSERT INTO conversations \
                 (id, project_id, task_id, title, provider, config, \
                  is_initial_conversation, last_interacted_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                input.project_id,
                input.task_id,
                trimmed,
                input.provider,
                input.config,
                is_initial as i32,
                now_iso,
            ],
        )?;
        drop(conn);

        self.get(&id)?
            .ok_or_else(|| ConversationsError::NotFound(id))
    }

    pub fn rename(&self, id: &str, title: &str) -> Result<(), ConversationsError> {
        let trimmed = title.trim();
        if trimmed.is_empty() {
            return Err(ConversationsError::EmptyTitle);
        }
        let conn = self.db.write()?;
        let affected = conn.execute(
            "UPDATE conversations SET title = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![trimmed, id],
        )?;
        if affected == 0 {
            return Err(ConversationsError::NotFound(id.into()));
        }
        Ok(())
    }

    /// Bump `last_interacted_at` to now. Used to keep tabs sorted by
    /// recency without a write on every PTY byte.
    pub fn touch(&self, id: &str) -> Result<(), ConversationsError> {
        let conn = self.db.write()?;
        let affected = conn.execute(
            "UPDATE conversations \
             SET last_interacted_at = ?1, updated_at = CURRENT_TIMESTAMP \
             WHERE id = ?2",
            params![chrono::Utc::now().to_rfc3339(), id],
        )?;
        if affected == 0 {
            return Err(ConversationsError::NotFound(id.into()));
        }
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), ConversationsError> {
        let conn = self.db.write()?;
        let affected =
            conn.execute("DELETE FROM conversations WHERE id = ?1", params![id])?;
        if affected == 0 {
            return Err(ConversationsError::NotFound(id.into()));
        }
        Ok(())
    }
}

fn row_to_conversation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Conversation> {
    let is_initial_int: i32 = row.get(6)?;
    Ok(Conversation {
        id: row.get(0)?,
        project_id: row.get(1)?,
        task_id: row.get(2)?,
        title: row.get(3)?,
        provider: row.get(4)?,
        config: row.get(5)?,
        is_initial_conversation: is_initial_int != 0,
        last_interacted_at: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}
