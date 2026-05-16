//! Workspaces domain — a workspace is one unit of work inside a project,
//! placed either in a git worktree under `.emdash-worktrees/<branch>/`
//! (`placement = 'worktree'`) or in the project directory itself
//! (`placement = 'local'`).
//!
//! Tauri-runtime-free. Broadcast/Tauri glue lives in
//! `commands::workspaces`.

pub mod fs_lock;
pub mod model;
pub mod service;
pub mod worktree;

pub use fs_lock::WorkspaceFsMutationLock;
pub use model::{
    NewWorkspaceInput, Workspace, WorkspacePlacement, WorkspaceSourceBranch, WorkspaceStatus,
    WorkspacesError,
};
pub use service::WorkspacesService;
