//! Domain types for the `projects` module.

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

use crate::db::DbError;

/// Projection of one row from the `projects` table that the renderer
/// actually consumes. Mirror of the columns used in the v1 CRUD surface.
/// Fields that exist in the schema but aren't needed yet
/// (`workspace_provider`, `base_ref`, `ssh_connection_id`) are
/// intentionally omitted — port them when a feature requires them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Error)]
pub enum ProjectsError {
    #[error("path is empty")]
    EmptyPath,
    #[error("path is not absolute: {0}")]
    NotAbsolute(String),
    #[error("path does not exist: {0}")]
    PathMissing(String),
    #[error("path is not a directory: {0}")]
    PathNotDirectory(String),
    #[error("a project already tracks this path: {0}")]
    DuplicatePath(String),
    #[error("project not found: {0}")]
    NotFound(String),
    #[error("db error: {0}")]
    Db(#[from] DbError),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}
