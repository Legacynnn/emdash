//! Tauri glue for the `terminals` namespace.

use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::terminals::{NewTerminalInput, Terminal, TerminalsError, TerminalsService};
use crate::ui_sync::{UiMutationEvent, UiSyncManager};

#[derive(Debug, Serialize, Type)]
pub struct TerminalsCommandError {
    pub code: TerminalsErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TerminalsErrorCode {
    NotFound,
    TaskNotFound,
    EmptyName,
    Storage,
}

impl From<TerminalsError> for TerminalsCommandError {
    fn from(e: TerminalsError) -> Self {
        let code = match &e {
            TerminalsError::NotFound(_) => TerminalsErrorCode::NotFound,
            TerminalsError::TaskNotFound(_) => TerminalsErrorCode::TaskNotFound,
            TerminalsError::EmptyName => TerminalsErrorCode::EmptyName,
            TerminalsError::Db(_) | TerminalsError::Sqlite(_) => TerminalsErrorCode::Storage,
        };
        Self {
            code,
            message: e.to_string(),
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn terminals_list_for_task(
    service: State<'_, Arc<TerminalsService>>,
    task_id: String,
) -> Result<Vec<Terminal>, TerminalsCommandError> {
    service.list_for_task(&task_id).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn terminals_create(
    service: State<'_, Arc<TerminalsService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    input: NewTerminalInput,
) -> Result<Terminal, TerminalsCommandError> {
    let project_id = input.project_id.clone();
    let task_id = input.task_id.clone();
    let term = service.create(input)?;
    manager.broadcast(UiMutationEvent::TerminalCreated {
        id: term.id.clone(),
        task_id,
        project_id,
    });
    Ok(term)
}

#[tauri::command]
#[specta::specta]
pub fn terminals_rename(
    service: State<'_, Arc<TerminalsService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    id: String,
    name: String,
) -> Result<(), TerminalsCommandError> {
    service.rename(&id, &name)?;
    if let Some(term) = service.get(&id)? {
        manager.broadcast(UiMutationEvent::TerminalUpdated {
            id: term.id,
            task_id: term.task_id,
            project_id: term.project_id,
        });
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn terminals_delete(
    service: State<'_, Arc<TerminalsService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    id: String,
) -> Result<(), TerminalsCommandError> {
    let target = service
        .get(&id)?
        .ok_or(TerminalsError::NotFound(id.clone()))
        .map_err(TerminalsCommandError::from)?;
    service.delete(&id)?;
    manager.broadcast(UiMutationEvent::TerminalDeleted {
        id: target.id,
        task_id: target.task_id,
        project_id: target.project_id,
    });
    Ok(())
}
