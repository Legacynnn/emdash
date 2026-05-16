//! Tauri glue for the `conversations` namespace.
//!
//! Same pattern as `commands::workspaces`: thin wrapper over the domain
//! service, error envelope, broadcast a `UiMutationEvent` after every
//! successful write so the renderer's workspace-view cache invalidates.

use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::conversations::{
    Conversation, ConversationsError, ConversationsService, NewConversationInput,
};
use crate::ui_sync::{UiMutationEvent, UiSyncManager};

#[derive(Debug, Serialize, Type)]
pub struct ConversationsCommandError {
    pub code: ConversationsErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ConversationsErrorCode {
    NotFound,
    WorkspaceNotFound,
    EmptyTitle,
    Storage,
}

impl From<ConversationsError> for ConversationsCommandError {
    fn from(e: ConversationsError) -> Self {
        let code = match &e {
            ConversationsError::NotFound(_) => ConversationsErrorCode::NotFound,
            ConversationsError::WorkspaceNotFound(_) => ConversationsErrorCode::WorkspaceNotFound,
            ConversationsError::EmptyTitle => ConversationsErrorCode::EmptyTitle,
            ConversationsError::Db(_) | ConversationsError::Sqlite(_) => {
                ConversationsErrorCode::Storage
            }
        };
        Self {
            code,
            message: e.to_string(),
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn conversations_list_for_workspace(
    service: State<'_, Arc<ConversationsService>>,
    workspace_id: String,
) -> Result<Vec<Conversation>, ConversationsCommandError> {
    service.list_for_workspace(&workspace_id).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn conversations_create(
    service: State<'_, Arc<ConversationsService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    input: NewConversationInput,
) -> Result<Conversation, ConversationsCommandError> {
    let project_id = input.project_id.clone();
    let workspace_id = input.workspace_id.clone();
    let conv = service.create(input)?;
    manager.broadcast(UiMutationEvent::ConversationCreated {
        id: conv.id.clone(),
        workspace_id,
        project_id,
    });
    Ok(conv)
}

#[tauri::command]
#[specta::specta]
pub fn conversations_rename(
    service: State<'_, Arc<ConversationsService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    id: String,
    title: String,
) -> Result<(), ConversationsCommandError> {
    service.rename(&id, &title)?;
    if let Some(conv) = service.get(&id)? {
        manager.broadcast(UiMutationEvent::ConversationUpdated {
            id: conv.id,
            workspace_id: conv.workspace_id,
            project_id: conv.project_id,
        });
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn conversations_touch(
    service: State<'_, Arc<ConversationsService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    id: String,
) -> Result<(), ConversationsCommandError> {
    service.touch(&id)?;
    if let Some(conv) = service.get(&id)? {
        manager.broadcast(UiMutationEvent::ConversationUpdated {
            id: conv.id,
            workspace_id: conv.workspace_id,
            project_id: conv.project_id,
        });
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn conversations_delete(
    service: State<'_, Arc<ConversationsService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    id: String,
) -> Result<(), ConversationsCommandError> {
    let target = service
        .get(&id)?
        .ok_or(ConversationsError::NotFound(id.clone()))
        .map_err(ConversationsCommandError::from)?;
    service.delete(&id)?;
    manager.broadcast(UiMutationEvent::ConversationDeleted {
        id: target.id,
        workspace_id: target.workspace_id,
        project_id: target.project_id,
    });
    Ok(())
}
