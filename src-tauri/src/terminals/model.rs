//! Domain types for the `terminals` module.

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

use crate::db::DbError;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Terminal {
    pub id: String,
    pub project_id: String,
    pub workspace_id: String,
    pub name: String,
    pub ssh: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Type)]
pub struct NewTerminalInput {
    pub project_id: String,
    pub workspace_id: String,
    pub name: String,
    pub ssh: Option<bool>,
}

#[derive(Debug, Error)]
pub enum TerminalsError {
    #[error("terminal not found: {0}")]
    NotFound(String),
    #[error("workspace not found: {0}")]
    WorkspaceNotFound(String),
    #[error("name is empty")]
    EmptyName,
    #[error("db error: {0}")]
    Db(#[from] DbError),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}
