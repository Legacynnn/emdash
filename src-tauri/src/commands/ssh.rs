//! Tauri glue for the SSH subsystem (EMD-10 v1).
//!
//! Mirrors the per-feature pattern from `commands::projects` /
//! `commands::github`: map domain errors to a serializable
//! `{code, message}` envelope; broadcast `UiMutationEvent` after
//! every successful write so the renderer's React Query cache
//! invalidates.

use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::ssh::{
    ConnectionState, ConnectionTestResult, NewSshConnection, SshClient, SshConnection,
    SshConnectionManager, SshConnectionStore, SshError,
};
use crate::ui_sync::{UiMutationEvent, UiSyncManager};

#[derive(Debug, Serialize, Type)]
pub struct SshCommandError {
    pub code: SshErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SshErrorCode {
    NotFound,
    InUse,
    Client,
    Unsupported,
    Storage,
}

impl From<SshError> for SshCommandError {
    fn from(e: SshError) -> Self {
        let message = e.to_string();
        let code = match e {
            SshError::NotFound(_) => SshErrorCode::NotFound,
            SshError::InUse(..) => SshErrorCode::InUse,
            SshError::Client(_) => SshErrorCode::Client,
            SshError::Unsupported(_) => SshErrorCode::Unsupported,
            SshError::Io(_)
            | SshError::Sqlite(_)
            | SshError::Db(_)
            | SshError::Secrets(_)
            | SshError::Json(_) => SshErrorCode::Storage,
        };
        Self { code, message }
    }
}

#[tauri::command]
#[specta::specta]
pub fn ssh_list_connections(
    store: State<'_, Arc<SshConnectionStore>>,
) -> Result<Vec<SshConnection>, SshCommandError> {
    store.list().map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn ssh_save_connection(
    store: State<'_, Arc<SshConnectionStore>>,
    ui: State<'_, Arc<UiSyncManager>>,
    payload: NewSshConnection,
) -> Result<SshConnection, SshCommandError> {
    let saved = store.save(payload)?;
    ui.broadcast(UiMutationEvent::SshConnectionSaved {
        id: saved.id.clone(),
    });
    Ok(saved)
}

#[tauri::command]
#[specta::specta]
pub fn ssh_rename_connection(
    store: State<'_, Arc<SshConnectionStore>>,
    ui: State<'_, Arc<UiSyncManager>>,
    id: String,
    name: String,
) -> Result<(), SshCommandError> {
    store.rename(&id, &name)?;
    ui.broadcast(UiMutationEvent::SshConnectionSaved { id });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn ssh_delete_connection(
    store: State<'_, Arc<SshConnectionStore>>,
    manager: State<'_, Arc<SshConnectionManager>>,
    ui: State<'_, Arc<UiSyncManager>>,
    id: String,
) -> Result<(), SshCommandError> {
    // Best-effort disconnect before clearing the row.
    if manager.is_connected(&id) {
        let _ = manager.disconnect(&id).await;
    }
    store.delete(&id)?;
    ui.broadcast(UiMutationEvent::SshConnectionDeleted { id });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn ssh_test_connection(
    payload: NewSshConnection,
) -> Result<ConnectionTestResult, SshCommandError> {
    // Build a transient SshConnection from the payload without
    // touching the DB or secrets store.
    let transient = SshConnection {
        id: payload
            .id
            .clone()
            .unwrap_or_else(|| "transient".to_string()),
        name: payload.name,
        host: payload.host,
        port: payload.port,
        username: payload.username,
        auth_type: payload.auth_type,
        private_key_path: payload.private_key_path,
        use_agent: payload.use_agent,
        worktrees_dir: payload.worktrees_dir,
        updated_at: chrono::Utc::now().to_rfc3339(),
    };
    Ok(SshClient::test_connection(&transient).await)
}

#[tauri::command]
#[specta::specta]
pub async fn ssh_connect(
    store: State<'_, Arc<SshConnectionStore>>,
    manager: State<'_, Arc<SshConnectionManager>>,
    ui: State<'_, Arc<UiSyncManager>>,
    id: String,
) -> Result<ConnectionState, SshCommandError> {
    let connection = store
        .get(&id)?
        .ok_or_else(|| SshError::NotFound(id.clone()))?;
    let _client = manager.connect(&connection).await?;
    let state = manager.get_state(&id);
    ui.broadcast(UiMutationEvent::SshConnectionStateChanged { id });
    Ok(state)
}

#[tauri::command]
#[specta::specta]
pub async fn ssh_disconnect(
    manager: State<'_, Arc<SshConnectionManager>>,
    ui: State<'_, Arc<UiSyncManager>>,
    id: String,
) -> Result<(), SshCommandError> {
    manager.disconnect(&id).await?;
    ui.broadcast(UiMutationEvent::SshConnectionStateChanged { id });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn ssh_get_state(
    manager: State<'_, Arc<SshConnectionManager>>,
    id: String,
) -> Result<ConnectionState, SshCommandError> {
    Ok(manager.get_state(&id))
}

#[tauri::command]
#[specta::specta]
pub async fn ssh_exec(
    store: State<'_, Arc<SshConnectionStore>>,
    manager: State<'_, Arc<SshConnectionManager>>,
    id: String,
    command: String,
) -> Result<String, SshCommandError> {
    let connection = store
        .get(&id)?
        .ok_or_else(|| SshError::NotFound(id.clone()))?;
    let client = manager.connect(&connection).await?;
    Ok(client.exec(&command).await?)
}
