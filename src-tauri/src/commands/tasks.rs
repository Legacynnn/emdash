//! Tauri glue for the tasks CRUD surface. Same pattern as
//! `commands::projects`: map domain errors to a `{code, message}`
//! envelope, broadcast a `UiMutationEvent` after every successful
//! write.

use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::tasks::{NewTaskInput, Task, TaskSourceBranch, TasksError, TasksService};
use crate::ui_sync::{UiMutationEvent, UiSyncManager};

#[derive(Debug, Serialize, Type)]
pub struct TasksCommandError {
    pub code: TasksErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TasksErrorCode {
    EmptyName,
    EmptyBranch,
    BranchAlreadyExists,
    ProjectNotFound,
    ProjectPathInvalid,
    NotFound,
    WorktreePathExists,
    WorktreeFailed,
    Git,
    Storage,
    Malformed,
}

impl From<TasksError> for TasksCommandError {
    fn from(e: TasksError) -> Self {
        let message = e.to_string();
        let code = match e {
            TasksError::EmptyName => TasksErrorCode::EmptyName,
            TasksError::EmptyBranch => TasksErrorCode::EmptyBranch,
            TasksError::BranchAlreadyExists(_) => TasksErrorCode::BranchAlreadyExists,
            TasksError::ProjectNotFound(_) => TasksErrorCode::ProjectNotFound,
            TasksError::ProjectPathInvalid(_) => TasksErrorCode::ProjectPathInvalid,
            TasksError::NotFound(_) => TasksErrorCode::NotFound,
            TasksError::WorktreePathExists(_) => TasksErrorCode::WorktreePathExists,
            TasksError::WorktreeFailed(_) => TasksErrorCode::WorktreeFailed,
            TasksError::Git(_) => TasksErrorCode::Git,
            TasksError::Db(_) | TasksError::Sqlite(_) => TasksErrorCode::Storage,
            TasksError::MalformedSourceBranch(_) => TasksErrorCode::Malformed,
        };
        Self { code, message }
    }
}

#[tauri::command]
#[specta::specta]
pub fn tasks_list(
    service: State<'_, Arc<TasksService>>,
    project_id: String,
) -> Result<Vec<Task>, TasksCommandError> {
    service.list(&project_id).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn tasks_create(
    service: State<'_, Arc<TasksService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    project_id: String,
    name: String,
    source_branch: TaskSourceBranch,
    task_branch: Option<String>,
) -> Result<Task, TasksCommandError> {
    let task = service.create(NewTaskInput {
        project_id: project_id.clone(),
        name,
        source_branch,
        task_branch,
    })?;
    manager.broadcast(UiMutationEvent::TaskCreated {
        id: task.id.clone(),
        project_id: task.project_id.clone(),
    });
    Ok(task)
}

#[tauri::command]
#[specta::specta]
pub fn tasks_delete(
    service: State<'_, Arc<TasksService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    id: String,
) -> Result<(), TasksCommandError> {
    // Resolve project_id before delete so the broadcast carries it
    // back to the renderer's dispatcher (the dispatch helper needs to
    // know which TaskStore to invalidate).
    let project_id = service
        .get(&id)?
        .ok_or(TasksError::NotFound(id.clone()))
        .map_err(TasksCommandError::from)?
        .project_id;

    service.delete(&id)?;
    manager.broadcast(UiMutationEvent::TaskDeleted { id, project_id });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn tasks_rename(
    service: State<'_, Arc<TasksService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    id: String,
    name: String,
) -> Result<Task, TasksCommandError> {
    let task = service.rename(&id, &name)?;
    manager.broadcast(UiMutationEvent::TaskUpdated {
        id: task.id.clone(),
        project_id: task.project_id.clone(),
    });
    Ok(task)
}

#[tauri::command]
#[specta::specta]
pub fn tasks_archive(
    service: State<'_, Arc<TasksService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    id: String,
) -> Result<Task, TasksCommandError> {
    let task = service.archive(&id)?;
    manager.broadcast(UiMutationEvent::TaskUpdated {
        id: task.id.clone(),
        project_id: task.project_id.clone(),
    });
    Ok(task)
}

#[tauri::command]
#[specta::specta]
pub fn tasks_restore(
    service: State<'_, Arc<TasksService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    id: String,
) -> Result<Task, TasksCommandError> {
    let task = service.restore(&id)?;
    manager.broadcast(UiMutationEvent::TaskUpdated {
        id: task.id.clone(),
        project_id: task.project_id.clone(),
    });
    Ok(task)
}

#[tauri::command]
#[specta::specta]
pub fn tasks_set_pinned(
    service: State<'_, Arc<TasksService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    id: String,
    pinned: bool,
) -> Result<(), TasksCommandError> {
    service.set_pinned(&id, pinned)?;
    if let Some(task) = service.get(&id)? {
        manager.broadcast(UiMutationEvent::TaskUpdated {
            id: task.id,
            project_id: task.project_id,
        });
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn tasks_update_linked_issue(
    service: State<'_, Arc<TasksService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    id: String,
    linked_issue: Option<String>,
) -> Result<(), TasksCommandError> {
    service.update_linked_issue(&id, linked_issue)?;
    if let Some(task) = service.get(&id)? {
        manager.broadcast(UiMutationEvent::TaskUpdated {
            id: task.id,
            project_id: task.project_id,
        });
    }
    Ok(())
}

/// Compute a short slugged name suggestion. Deterministic — the same
/// `description` always produces the same suggestion. Useful for the
/// renderer's "Generate name" button on new-task dialogs.
#[tauri::command]
#[specta::specta]
pub fn tasks_generate_name(description: String) -> String {
    let trimmed = description.trim();
    if trimmed.is_empty() {
        return "untitled-task".into();
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
        "untitled-task".into()
    } else {
        cleaned.chars().take(48).collect()
    }
}
