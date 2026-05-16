//! Domain types for the `tasks` module.

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

use crate::db::DbError;
use crate::git::GitError;

/// Renderer-facing task projection. Fields beyond v1 (`task_branch`,
/// `linked_issue`, etc.) live in the DB schema but stay out of this
/// struct until a feature needs them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub status: TaskStatus,
    pub path: String,
    pub source_branch: TaskSourceBranch,
    pub pty_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Active,
    Archived,
}

/// Where the task's worktree branches from. JSON-encoded in the
/// `source_branch` column; the domain layer is the only thing that
/// touches the wire format. Schema migration 0002/0003 from Drizzle
/// history was about turning a plain string into this discriminator —
/// in emdash-dev's collapsed bootstrap it ships tagged from day one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskSourceBranch {
    Local { branch: String },
    Remote { host: String, branch: String },
}

impl TaskSourceBranch {
    /// Branch name to hand to `git worktree add`. For local, the branch
    /// is used as-is; for remote, the convention is `<host>/<branch>`,
    /// matching what `git fetch` writes under `refs/remotes/<host>/`.
    pub fn checkout_target(&self) -> String {
        match self {
            Self::Local { branch } => branch.clone(),
            Self::Remote { host, branch } => format!("{host}/{branch}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct NewTaskInput {
    pub project_id: String,
    pub name: String,
    pub source_branch: TaskSourceBranch,
    /// Branch name chosen by the renderer (e.g. "feat/add-search").
    /// `None` falls back to a slug derived from `name`. Either way,
    /// no `task/` prefix and no UUID suffix are added by the service.
    pub task_branch: Option<String>,
}

#[derive(Debug, Error)]
pub enum TasksError {
    #[error("project not found: {0}")]
    ProjectNotFound(String),
    #[error("task not found: {0}")]
    NotFound(String),
    #[error("name is empty")]
    EmptyName,
    #[error("project path is not a directory: {0}")]
    ProjectPathInvalid(String),
    #[error("worktree path already exists: {0}")]
    WorktreePathExists(String),
    #[error("branch already exists: {0}")]
    BranchAlreadyExists(String),
    #[error("branch name is empty")]
    EmptyBranch,
    #[error("git worktree command failed: {0}")]
    WorktreeFailed(String),
    #[error("git error: {0}")]
    Git(#[from] GitError),
    #[error("db error: {0}")]
    Db(#[from] DbError),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("malformed source_branch JSON: {0}")]
    MalformedSourceBranch(String),
}
