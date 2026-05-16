//! Tauri glue for the workspaces CRUD surface. Same pattern as
//! `commands::projects`: map domain errors to a `{code, message}`
//! envelope, broadcast a `UiMutationEvent` after every successful
//! write.

use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::ui_sync::{UiMutationEvent, UiSyncManager};
use crate::workspaces::{
    NewWorkspaceInput, Workspace, WorkspacePlacement, WorkspaceSourceBranch, WorkspacesError,
    WorkspacesService,
};

#[derive(Debug, Serialize, Type)]
pub struct WorkspacesCommandError {
    pub code: WorkspacesErrorCode,
    pub message: String,
    /// Populated for `LocalSlotTaken` — surfaces which workspace is occupying the slot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_workspace_id: Option<String>,
    /// Populated for `DirtyTree`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed_files: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum WorkspacesErrorCode {
    EmptyName,
    EmptyBranch,
    BranchAlreadyExists,
    BranchNotFound,
    ProjectNotFound,
    ProjectPathInvalid,
    NotFound,
    WorktreePathExists,
    WorktreeFailed,
    SwitchFailed,
    DirtyTree,
    LocalSlotTaken,
    Git,
    Storage,
    Malformed,
}

impl From<WorkspacesError> for WorkspacesCommandError {
    fn from(e: WorkspacesError) -> Self {
        let message = e.to_string();
        let (code, existing_workspace_id, changed_files) = match e {
            WorkspacesError::EmptyName => (WorkspacesErrorCode::EmptyName, None, None),
            WorkspacesError::EmptyBranch => (WorkspacesErrorCode::EmptyBranch, None, None),
            WorkspacesError::BranchAlreadyExists(_) => {
                (WorkspacesErrorCode::BranchAlreadyExists, None, None)
            }
            WorkspacesError::BranchNotFound(_) => (WorkspacesErrorCode::BranchNotFound, None, None),
            WorkspacesError::ProjectNotFound(_) => {
                (WorkspacesErrorCode::ProjectNotFound, None, None)
            }
            WorkspacesError::ProjectPathInvalid(_) => {
                (WorkspacesErrorCode::ProjectPathInvalid, None, None)
            }
            WorkspacesError::NotFound(_) => (WorkspacesErrorCode::NotFound, None, None),
            WorkspacesError::WorktreePathExists(_) => {
                (WorkspacesErrorCode::WorktreePathExists, None, None)
            }
            WorkspacesError::WorktreeFailed(_) => (WorkspacesErrorCode::WorktreeFailed, None, None),
            WorkspacesError::SwitchFailed(_) => (WorkspacesErrorCode::SwitchFailed, None, None),
            WorkspacesError::DirtyTree { changed_files } => {
                (WorkspacesErrorCode::DirtyTree, None, Some(changed_files))
            }
            WorkspacesError::LocalSlotTaken {
                existing_workspace_id,
                ..
            } => (
                WorkspacesErrorCode::LocalSlotTaken,
                Some(existing_workspace_id),
                None,
            ),
            WorkspacesError::Git(_) => (WorkspacesErrorCode::Git, None, None),
            WorkspacesError::Db(_) | WorkspacesError::Sqlite(_) => {
                (WorkspacesErrorCode::Storage, None, None)
            }
            WorkspacesError::MalformedSourceBranch(_) => {
                (WorkspacesErrorCode::Malformed, None, None)
            }
        };
        Self {
            code,
            message,
            existing_workspace_id,
            changed_files,
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn workspaces_list(
    service: State<'_, Arc<WorkspacesService>>,
    project_id: String,
) -> Result<Vec<Workspace>, WorkspacesCommandError> {
    service.list(&project_id).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn workspaces_create(
    service: State<'_, Arc<WorkspacesService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    project_id: String,
    name: String,
    source_branch: WorkspaceSourceBranch,
    workspace_branch: Option<String>,
    placement: WorkspacePlacement,
    existing_branch: bool,
) -> Result<Workspace, WorkspacesCommandError> {
    let workspace = service.create(NewWorkspaceInput {
        project_id: project_id.clone(),
        name,
        source_branch,
        workspace_branch,
        placement,
        existing_branch,
    })?;
    manager.broadcast(UiMutationEvent::WorkspaceCreated {
        id: workspace.id.clone(),
        project_id: workspace.project_id.clone(),
    });
    Ok(workspace)
}

#[tauri::command]
#[specta::specta]
pub fn workspaces_delete(
    service: State<'_, Arc<WorkspacesService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    id: String,
) -> Result<(), WorkspacesCommandError> {
    let project_id = service
        .get(&id)?
        .ok_or(WorkspacesError::NotFound(id.clone()))
        .map_err(WorkspacesCommandError::from)?
        .project_id;
    service.delete(&id)?;
    manager.broadcast(UiMutationEvent::WorkspaceDeleted { id, project_id });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn workspaces_rename(
    service: State<'_, Arc<WorkspacesService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    id: String,
    name: String,
) -> Result<Workspace, WorkspacesCommandError> {
    let workspace = service.rename(&id, &name)?;
    manager.broadcast(UiMutationEvent::WorkspaceUpdated {
        id: workspace.id.clone(),
        project_id: workspace.project_id.clone(),
    });
    Ok(workspace)
}

#[tauri::command]
#[specta::specta]
pub fn workspaces_archive(
    service: State<'_, Arc<WorkspacesService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    id: String,
) -> Result<Workspace, WorkspacesCommandError> {
    let workspace = service.archive(&id)?;
    manager.broadcast(UiMutationEvent::WorkspaceUpdated {
        id: workspace.id.clone(),
        project_id: workspace.project_id.clone(),
    });
    Ok(workspace)
}

#[tauri::command]
#[specta::specta]
pub fn workspaces_restore(
    service: State<'_, Arc<WorkspacesService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    id: String,
) -> Result<Workspace, WorkspacesCommandError> {
    let workspace = service.restore(&id)?;
    manager.broadcast(UiMutationEvent::WorkspaceUpdated {
        id: workspace.id.clone(),
        project_id: workspace.project_id.clone(),
    });
    Ok(workspace)
}

#[tauri::command]
#[specta::specta]
pub fn workspaces_set_pinned(
    service: State<'_, Arc<WorkspacesService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    id: String,
    pinned: bool,
) -> Result<(), WorkspacesCommandError> {
    service.set_pinned(&id, pinned)?;
    if let Some(workspace) = service.get(&id)? {
        manager.broadcast(UiMutationEvent::WorkspaceUpdated {
            id: workspace.id,
            project_id: workspace.project_id,
        });
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn workspaces_update_linked_issue(
    service: State<'_, Arc<WorkspacesService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    id: String,
    linked_issue: Option<String>,
) -> Result<(), WorkspacesCommandError> {
    service.update_linked_issue(&id, linked_issue)?;
    if let Some(workspace) = service.get(&id)? {
        manager.broadcast(UiMutationEvent::WorkspaceUpdated {
            id: workspace.id,
            project_id: workspace.project_id,
        });
    }
    Ok(())
}

/// Compute a short slugged name suggestion. Deterministic — the same
/// `description` always produces the same suggestion. Useful for the
/// renderer's "Generate name" button on new-workspace dialogs.
#[tauri::command]
#[specta::specta]
pub fn workspaces_generate_name(description: String) -> String {
    let trimmed = description.trim();
    if trimmed.is_empty() {
        return "untitled-workspace".into();
    }
    let mut out = String::new();
    let mut last_was_dash = false;
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash && !out.is_empty() {
            out.push('-');
            last_was_dash = true;
        }
    }
    let cleaned = out.trim_matches('-');
    if cleaned.is_empty() {
        "untitled-workspace".into()
    } else {
        cleaned.chars().take(48).collect()
    }
}
