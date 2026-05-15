//! Tauri glue for the telemetry pipeline. Exposes the user toggle
//! and the `app.focus / app.unfocus / app.dau_ping / user.identify`
//! events.
//!
//! The toggle is stored in `app_settings`; the runtime layer
//! ([`crate::telemetry`]) re-reads the toggle on every `record` call
//! so we don't need a separate notification path.

use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::db::Db;
use crate::telemetry::{self, Telemetry, TelemetryEvent, TelemetryProps};

#[derive(Debug, Serialize, Type)]
pub struct TelemetryCommandError {
    pub code: TelemetryErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryErrorCode {
    Storage,
    Invalid,
}

#[tauri::command]
#[specta::specta]
pub fn telemetry_get_enabled(db: State<'_, Arc<Db>>) -> Result<bool, TelemetryCommandError> {
    telemetry::get_enabled(&db).map_err(|e| TelemetryCommandError {
        code: TelemetryErrorCode::Storage,
        message: e.to_string(),
    })
}

#[tauri::command]
#[specta::specta]
pub fn telemetry_set_enabled(
    db: State<'_, Arc<Db>>,
    enabled: bool,
) -> Result<(), TelemetryCommandError> {
    telemetry::set_enabled(&db, enabled).map_err(|e| TelemetryCommandError {
        code: TelemetryErrorCode::Storage,
        message: e.to_string(),
    })
}

/// Record one of the standard app events. `user_identify` carries
/// optional GitHub profile fields; the rest take no payload. All
/// gates (compile-time host, user toggle) live in the runtime.
#[tauri::command]
#[specta::specta]
pub fn telemetry_record(
    telemetry: State<'_, Arc<Telemetry>>,
    event: TelemetryEvent,
    gh_username: Option<String>,
    gh_account_id: Option<String>,
    email: Option<String>,
) -> Result<(), TelemetryCommandError> {
    let mut props = TelemetryProps::new();
    if matches!(event, TelemetryEvent::UserIdentify) {
        if let Some(u) = gh_username {
            props.insert("gh_username".into(), serde_json::Value::String(u));
        }
        if let Some(a) = gh_account_id {
            props.insert("gh_account_id".into(), serde_json::Value::String(a));
        }
        if let Some(e) = email {
            props.insert("email".into(), serde_json::Value::String(e));
        }
    }
    telemetry
        .record(event, props)
        .map_err(|e| TelemetryCommandError {
            code: TelemetryErrorCode::Storage,
            message: e.to_string(),
        })
}
