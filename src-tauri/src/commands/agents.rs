//! Tauri glue for `agents.start / agents.stop` (EMD-27).
//!
//! Hooks the runtime-free `AgentService` into:
//! - `Channel<Vec<u8>>` for streaming the agent's PTY bytes to the
//!   renderer's xterm.js
//! - `UiSyncManager` for `AgentStarted` / `AgentExited` broadcasts

use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use tauri::{ipc::Channel, State};

use crate::agents::{provider, AgentProvider, AgentService, AgentsError};
use crate::pty::types::{PtyId, PtySize};
use crate::ui_sync::{UiMutationEvent, UiSyncManager};

#[derive(Debug, Serialize, Type)]
pub struct AgentsCommandError {
    pub code: AgentsErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AgentsErrorCode {
    TaskNotFound,
    AlreadyRunning,
    SpawnFailed,
    Storage,
}

impl From<AgentsError> for AgentsCommandError {
    fn from(e: AgentsError) -> Self {
        let code = match &e {
            AgentsError::TaskNotFound(_) => AgentsErrorCode::TaskNotFound,
            AgentsError::AlreadyRunning(_) => AgentsErrorCode::AlreadyRunning,
            AgentsError::Pty(_) => AgentsErrorCode::SpawnFailed,
            AgentsError::Tasks(_) | AgentsError::Db(_) | AgentsError::Sqlite(_) => {
                AgentsErrorCode::Storage
            }
        };
        Self {
            code,
            message: e.to_string(),
        }
    }
}

/// Spawn `provider` for `task_id`. Returns the PTY id so the
/// renderer can wire `pty_write` / `pty_resize` / `pty_kill` to the
/// same id without a follow-up lookup.
#[tauri::command]
#[specta::specta]
pub async fn agents_start(
    service: State<'_, Arc<AgentService>>,
    ui_sync: State<'_, Arc<UiSyncManager>>,
    task_id: String,
    provider: AgentProvider,
    size: PtySize,
    on_output: Channel<Vec<u8>>,
) -> Result<PtyId, AgentsCommandError> {
    let svc = service.inner().clone();
    let task_id_clone = task_id.clone();
    let ui_sync = ui_sync.inner().clone();

    // PTY callback fires from the reader thread; clone the channel
    // so it lives as long as the underlying spawn does.
    let cb: crate::agents::service::OutputCallback = Arc::new(move |bytes| {
        let _ = on_output.send(bytes);
    });

    let result = tokio::task::spawn_blocking(move || svc.start(&task_id, provider, size, cb))
        .await
        .map_err(|e| AgentsCommandError {
            code: AgentsErrorCode::SpawnFailed,
            message: format!("join: {e}"),
        })?;

    let pty_id = result.map_err(AgentsCommandError::from)?;
    ui_sync.broadcast(UiMutationEvent::AgentStarted {
        task_id: task_id_clone,
        provider,
    });
    Ok(pty_id)
}

#[tauri::command]
#[specta::specta]
pub async fn agents_stop(
    service: State<'_, Arc<AgentService>>,
    ui_sync: State<'_, Arc<UiSyncManager>>,
    task_id: String,
) -> Result<(), AgentsCommandError> {
    let svc = service.inner().clone();
    let tid = task_id.clone();
    tokio::task::spawn_blocking(move || svc.stop(&task_id))
        .await
        .map_err(|e| AgentsCommandError {
            code: AgentsErrorCode::SpawnFailed,
            message: format!("join: {e}"),
        })??;
    ui_sync.broadcast(UiMutationEvent::AgentExited {
        task_id: tid,
        exit_code: None,
    });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn agents_list_providers() -> Vec<provider::ProviderSpec> {
    provider::known_providers()
}
