//! Resource-monitor commands. Returns a renderer-shaped envelope
//! (`{ success, data: { cpuCount, app, entries } }`). Until a real
//! per-PTY sampler lands, the host returns a sentinel
//! `{ success: false }` shape — the renderer's resource-monitor store
//! treats that as "no sample yet" and keeps the badge idle.

use serde::Serialize;
use specta::Type;

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSnapshot {
    pub success: bool,
}

#[tauri::command]
#[specta::specta]
pub fn resource_monitor_get_snapshot() -> ResourceSnapshot {
    ResourceSnapshot { success: false }
}
