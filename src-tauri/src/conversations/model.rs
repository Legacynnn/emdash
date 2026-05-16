//! Domain types for the `conversations` module.

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

use crate::db::DbError;

/// Projection of a `conversations` row that the renderer consumes. The
/// `config` column stores agent-specific JSON (e.g. `{ "autoApprove":
/// true }` for claude); we surface it as an opaque string and let the
/// renderer decode where needed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Conversation {
    pub id: String,
    pub project_id: String,
    pub task_id: String,
    pub title: String,
    pub provider: Option<String>,
    pub config: Option<String>,
    pub is_initial_conversation: bool,
    pub last_interacted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Type)]
pub struct NewConversationInput {
    pub project_id: String,
    pub task_id: String,
    pub title: String,
    pub provider: Option<String>,
    pub config: Option<String>,
    pub is_initial_conversation: Option<bool>,
}

#[derive(Debug, Error)]
pub enum ConversationsError {
    #[error("conversation not found: {0}")]
    NotFound(String),
    #[error("task not found: {0}")]
    TaskNotFound(String),
    #[error("title is empty")]
    EmptyTitle,
    #[error("db error: {0}")]
    Db(#[from] DbError),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}
