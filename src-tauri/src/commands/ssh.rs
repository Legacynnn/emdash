//! SSH connection registry. Tauri's SSH layer is unported (the
//! Electron implementation used ssh2 + a complex connection manager
//! marked high-risk in AGENTS.md). Host returns empty lists so the
//! renderer's SSH view degrades to "no remote connections" — the
//! local-only flow is fully functional without this.

use serde::Serialize;
use specta::Type;

#[derive(Debug, Serialize, Type)]
pub struct SshConnection {
    pub id: String,
    pub name: String,
    pub host: String,
    pub username: String,
}

#[tauri::command]
#[specta::specta]
pub fn ssh_get_connections() -> Vec<SshConnection> {
    Vec::new()
}

#[tauri::command]
#[specta::specta]
pub fn ssh_get_connection_state() -> std::collections::HashMap<String, String> {
    std::collections::HashMap::new()
}

#[tauri::command]
#[specta::specta]
pub fn ssh_get_health_states() -> std::collections::HashMap<String, String> {
    std::collections::HashMap::new()
}
