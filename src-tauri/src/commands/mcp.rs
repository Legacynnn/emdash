//! MCP Tauri glue. Wraps `McpService` and maps `McpError` onto a
//! serializable envelope the renderer expects.

use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::mcp::{
    McpError, McpLoadAllResponse, McpProviderInfo, McpServer, McpService,
};
use crate::ui_sync::{UiMutationEvent, UiSyncManager};

#[derive(Debug, Serialize, Type)]
pub struct McpCommandError {
    pub code: McpErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum McpErrorCode {
    InvalidName,
    Io,
    Catalog,
    ConfigParse,
    PartialWrite,
}

impl From<McpError> for McpCommandError {
    fn from(e: McpError) -> Self {
        let message = e.to_string();
        let code = match e {
            McpError::InvalidName(_) => McpErrorCode::InvalidName,
            McpError::Io { .. } => McpErrorCode::Io,
            McpError::CatalogParse(_) => McpErrorCode::Catalog,
            McpError::ConfigParse { .. } => McpErrorCode::ConfigParse,
            McpError::PartialWrite(_) => McpErrorCode::PartialWrite,
        };
        Self { code, message }
    }
}

#[tauri::command]
#[specta::specta]
pub fn mcp_load_all(
    service: State<'_, Arc<McpService>>,
) -> Result<McpLoadAllResponse, McpCommandError> {
    service.load_all().map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn mcp_get_providers(service: State<'_, Arc<McpService>>) -> Vec<McpProviderInfo> {
    service.get_providers()
}

#[tauri::command]
#[specta::specta]
pub fn mcp_refresh_providers(
    service: State<'_, Arc<McpService>>,
    manager: State<'_, Arc<UiSyncManager>>,
) -> Vec<McpProviderInfo> {
    let out = service.get_providers();
    manager.broadcast(UiMutationEvent::McpProvidersRefreshed);
    out
}

#[tauri::command]
#[specta::specta]
pub fn mcp_save_server(
    service: State<'_, Arc<McpService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    server: McpServer,
) -> Result<(), McpCommandError> {
    service.save_server(&server)?;
    manager.broadcast(UiMutationEvent::McpServerSaved {
        name: server.name.clone(),
    });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn mcp_remove_server(
    service: State<'_, Arc<McpService>>,
    manager: State<'_, Arc<UiSyncManager>>,
    server_name: String,
) -> Result<(), McpCommandError> {
    service.remove_server(&server_name)?;
    manager.broadcast(UiMutationEvent::McpServerRemoved {
        name: server_name,
    });
    Ok(())
}
