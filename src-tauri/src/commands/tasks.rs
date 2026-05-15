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
) -> Result<Task, TasksCommandError> {
    let task = service.create(NewTaskInput {
        project_id: project_id.clone(),
        name,
        source_branch,
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
