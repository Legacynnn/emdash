//! Tauri glue for the projects CRUD surface. Maps `ProjectsError` to a
//! serializable `ProjectsCommandError` envelope and broadcasts the
//! relevant `UiMutationEvent` after each successful write.
//!
//! Pattern reference for every future feature PR (see EMD-7 acceptance
//! criteria and `docs/decisions/0004-ui-mutation-event-bridge.md`).

use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::projects::{Project, ProjectsError, ProjectsService};
use crate::ui_sync::{UiMutationEvent, UiSyncManager};

#[derive(Debug, Serialize, Type)]
pub struct ProjectsCommandError {
    /// Stable machine-readable identifier; the renderer matches on this.
    pub code: ProjectsErrorCode,
    /// Human-readable hint. Surface to the user verbatim.
    pub message: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProjectsErrorCode {
    EmptyPath,
    NotAbsolute,
    PathMissing,
    PathNotDirectory,
    DuplicatePath,
    NotFound,
    Storage,
}

impl From<ProjectsError> for ProjectsCommandError {
    fn from(e: ProjectsError) -> Self {
        let message = e.to_string();
        let code = match e {
            ProjectsError::EmptyPath => ProjectsErrorCode::EmptyPath,
            ProjectsError::NotAbsolute(_) => ProjectsErrorCode::NotAbsolute,
            ProjectsError::PathMissing(_) => ProjectsErrorCode::PathMissing,
            ProjectsError::PathNotDirectory(_) => ProjectsErrorCode::PathNotDirectory,
            ProjectsError::DuplicatePath(_) => ProjectsErrorCode::DuplicatePath,
            ProjectsError::NotFound(_) => ProjectsErrorCode::NotFound,
            ProjectsError::Db(_) | ProjectsError::Sqlite(_) => ProjectsErrorCode::Storage,
        };
        Self { code, message }
    }
}

#[tauri::command]
#[specta::specta]
pub fn projects_list(
    service: State<'_, Arc<ProjectsService>>,
) -> Result<Vec<Project>, ProjectsCommandError> {
    service.list().map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn projects_add(
    service: State<'_, Arc<ProjectsService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    path: String,
) -> Result<Project, ProjectsCommandError> {
    let project = service.add(&path)?;
    manager.broadcast(UiMutationEvent::ProjectCreated {
        id: project.id.clone(),
    });
    Ok(project)
}

#[tauri::command]
#[specta::specta]
pub fn projects_remove(
    service: State<'_, Arc<ProjectsService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    id: String,
) -> Result<(), ProjectsCommandError> {
    service.remove(&id)?;
    manager.broadcast(UiMutationEvent::ProjectDeleted { id });
    Ok(())
}
