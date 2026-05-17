//! Domain types for the `workspaces` module.

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

use crate::db::DbError;
use crate::git::GitError;

/// Renderer-facing workspace projection. Fields beyond v1
/// (`workspace_branch`, `linked_issue`, etc.) live in the DB schema but
/// stay out of this struct until a feature needs them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Workspace {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub status: WorkspaceStatus,
    pub placement: WorkspacePlacement,
    pub path: String,
    pub source_branch: WorkspaceSourceBranch,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceStatus {
    Active,
    Archived,
}

/// Placement controls how/where the workspace lives on disk.
///
/// - `worktree`: a `git worktree` rooted at `.emdash-worktrees/<branch>/`.
/// - `local`: the project directory itself. At most one active local
///   workspace per project (enforced by a partial-unique DB index).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum WorkspacePlacement {
    Worktree,
    Local,
}

/// Where the workspace's branch is based on. JSON-encoded in the
/// `source_branch` column; the domain layer is the only thing that
/// touches the wire format.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkspaceSourceBranch {
    Local { branch: String },
    Remote { host: String, branch: String },
}

impl WorkspaceSourceBranch {
    /// Branch name to hand to `git worktree add` or `git switch`. For
    /// local, the branch is used as-is; for remote, the convention is
    /// `<host>/<branch>`, matching what `git fetch` writes under
    /// `refs/remotes/<host>/`.
    pub fn checkout_target(&self) -> String {
        match self {
            Self::Local { branch } => branch.clone(),
            Self::Remote { host, branch } => format!("{host}/{branch}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct NewWorkspaceInput {
    pub project_id: String,
    pub name: String,
    pub source_branch: WorkspaceSourceBranch,
    /// Branch name chosen by the renderer (e.g. "feat/add-search").
    /// `None` falls back to a slug derived from `name`. Either way,
    /// no UUID suffix is added by the service.
    pub workspace_branch: Option<String>,
    pub placement: WorkspacePlacement,
    /// Only meaningful when `placement = Local`: if true, the renderer
    /// has picked an existing branch to switch into; if false, a new
    /// branch is created from `source_branch`.
    pub existing_branch: bool,
}

#[derive(Debug, Error)]
pub enum WorkspacesError {
    #[error("project not found: {0}")]
    ProjectNotFound(String),
    #[error("workspace not found: {0}")]
    NotFound(String),
    #[error("name is empty")]
    EmptyName,
    #[error("project path is not a directory: {0}")]
    ProjectPathInvalid(String),
    #[error("worktree path already exists: {0}")]
    WorktreePathExists(String),
    #[error("branch already exists: {0}")]
    BranchAlreadyExists(String),
    #[error("branch not found: {0}")]
    BranchNotFound(String),
    #[error("branch name is empty")]
    EmptyBranch,
    #[error("git worktree command failed: {0}")]
    WorktreeFailed(String),
    #[error("git switch failed: {0}")]
    SwitchFailed(String),
    #[error("project working tree is dirty")]
    DirtyTree { changed_files: Vec<String> },
    #[error("project already has an active local workspace: {existing_workspace_name}")]
    LocalSlotTaken {
        existing_workspace_id: String,
        existing_workspace_name: String,
    },
    #[error("git error: {0}")]
    Git(#[from] GitError),
    #[error("db error: {0}")]
    Db(#[from] DbError),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("malformed source_branch JSON: {0}")]
    MalformedSourceBranch(String),
}
