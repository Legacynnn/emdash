//! Tauri glue for the `editor_buffers` domain (ADR-0012).

use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::db::Db;
use crate::editor_buffers::{self, EditorBuffer, EditorBuffersError};

#[derive(Debug, Serialize, Type)]
pub struct EditorBuffersCommandError {
    pub code: EditorBuffersErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum EditorBuffersErrorCode {
    Storage,
}

impl From<EditorBuffersError> for EditorBuffersCommandError {
    fn from(e: EditorBuffersError) -> Self {
        Self {
            code: EditorBuffersErrorCode::Storage,
            message: e.to_string(),
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn editor_buffer_save(
    db: State<'_, Arc<Db>>,
    project_id: String,
    workspace_id: String,
    file_path: String,
    content: String,
) -> Result<EditorBuffer, EditorBuffersCommandError> {
    editor_buffers::save(&db, &project_id, &workspace_id, &file_path, &content).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn editor_buffer_clear(
    db: State<'_, Arc<Db>>,
    project_id: String,
    workspace_id: String,
    file_path: String,
) -> Result<(), EditorBuffersCommandError> {
    editor_buffers::clear(&db, &project_id, &workspace_id, &file_path).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn editor_buffer_list(
    db: State<'_, Arc<Db>>,
    project_id: String,
    workspace_id: String,
) -> Result<Vec<EditorBuffer>, EditorBuffersCommandError> {
    editor_buffers::list(&db, &project_id, &workspace_id).map_err(Into::into)
}
