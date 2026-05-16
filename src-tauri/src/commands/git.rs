//! Tauri glue for read-only git operations. Maps the renderer's
//! `(projectId, workspaceId)` arg pair onto an absolute worktree path
//! looked up via `TasksService::get`, then defers to the underlying
//! `git2`-backed module in `crate::git::ops`.
//!
//! Mutating ops (commit/push/pull/stage/revert) are not yet ported.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::git::ops::{
    current_branch, diff_against_head, list_branches, status_porcelain, GitError, GitStatusEntry,
};
use crate::tasks::TasksService;

#[derive(Debug, Serialize, Type)]
pub struct GitCommandError {
    pub code: GitErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GitErrorCode {
    NotFound,
    NotARepository,
    GitError,
}

impl From<GitError> for GitCommandError {
    fn from(e: GitError) -> Self {
        let code = match &e {
            GitError::NotARepository(_) => GitErrorCode::NotARepository,
            GitError::RefNotFound(_) | GitError::ObjectNotFound(_) => GitErrorCode::NotFound,
            GitError::Git2(_) => GitErrorCode::GitError,
        };
        Self {
            code,
            message: e.to_string(),
        }
    }
}

fn resolve_workspace_path(
    service: &TasksService,
    workspace_id: &str,
) -> Result<PathBuf, GitCommandError> {
    match service.get(workspace_id).map_err(|e| GitCommandError {
        code: GitErrorCode::NotFound,
        message: e.to_string(),
    })? {
        Some(task) => Ok(PathBuf::from(task.path)),
        None => Err(GitCommandError {
            code: GitErrorCode::NotFound,
            message: format!("workspace not found: {workspace_id}"),
        }),
    }
}

/// Renderer-facing combined status payload. Mirrors the shape produced
/// by the Electron `GitFs::getFullStatus` so the renderer can consume
/// either backend transparently.
#[derive(Debug, Serialize, Type)]
pub struct GitFullStatus {
    pub current_branch: Option<String>,
    pub changes: Vec<GitStatusEntry>,
}

#[tauri::command]
#[specta::specta]
pub fn git_full_status(
    service: State<'_, Arc<TasksService>>,
    workspace_id: String,
) -> Result<GitFullStatus, GitCommandError> {
    let path = resolve_workspace_path(&service, &workspace_id)?;
    let branch = current_branch(&path).map_err(GitCommandError::from)?;
    let changes = status_porcelain(&path).map_err(GitCommandError::from)?;
    Ok(GitFullStatus {
        current_branch: branch,
        changes,
    })
}

#[tauri::command]
#[specta::specta]
pub fn git_changed_files(
    service: State<'_, Arc<TasksService>>,
    workspace_id: String,
) -> Result<Vec<GitStatusEntry>, GitCommandError> {
    let path = resolve_workspace_path(&service, &workspace_id)?;
    status_porcelain(&path).map_err(GitCommandError::from)
}

#[tauri::command]
#[specta::specta]
pub fn git_current_branch(
    service: State<'_, Arc<TasksService>>,
    workspace_id: String,
) -> Result<Option<String>, GitCommandError> {
    let path = resolve_workspace_path(&service, &workspace_id)?;
    current_branch(&path).map_err(GitCommandError::from)
}

#[tauri::command]
#[specta::specta]
pub fn git_diff_against_head(
    service: State<'_, Arc<TasksService>>,
    workspace_id: String,
) -> Result<String, GitCommandError> {
    let path = resolve_workspace_path(&service, &workspace_id)?;
    diff_against_head(&path).map_err(GitCommandError::from)
}

#[tauri::command]
#[specta::specta]
pub fn git_list_branches(
    service: State<'_, Arc<TasksService>>,
    workspace_id: String,
) -> Result<Vec<String>, GitCommandError> {
    let path = resolve_workspace_path(&service, &workspace_id)?;
    list_branches(&path).map_err(GitCommandError::from)
}
