//! Tauri glue for the `view_state` domain (ADR-0018).
//!
//! Values cross the IPC boundary as JSON-encoded strings — specta's
//! `serde_json::Value` support depends on an off-by-default feature
//! we'd rather not pin. Renderer parses on receive; small price for
//! a stable types-pipeline.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::db::Db;
use crate::view_state::{self, ViewStateError};

#[derive(Debug, Serialize, Type)]
pub struct ViewStateCommandError {
    pub code: ViewStateErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ViewStateErrorCode {
    Storage,
    Malformed,
}

impl From<ViewStateError> for ViewStateCommandError {
    fn from(e: ViewStateError) -> Self {
        let code = match e {
            ViewStateError::MalformedJson(_) => ViewStateErrorCode::Malformed,
            _ => ViewStateErrorCode::Storage,
        };
        let message = e.to_string();
        Self { code, message }
    }
}

// `value_json` is a JSON-encoded payload. Anything that round-trips
// through `JSON.stringify` / `JSON.parse` is valid.
#[tauri::command]
#[specta::specta]
pub fn view_state_save(
    db: State<'_, Arc<Db>>,
    key: String,
    value_json: String,
) -> Result<(), ViewStateCommandError> {
    let parsed: serde_json::Value =
        serde_json::from_str(&value_json).map_err(|e| ViewStateCommandError {
            code: ViewStateErrorCode::Malformed,
            message: format!("not valid JSON: {e}"),
        })?;
    view_state::save(&db, &key, &parsed).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn view_state_get(
    db: State<'_, Arc<Db>>,
    key: String,
) -> Result<Option<String>, ViewStateCommandError> {
    let value = view_state::get(&db, &key)?;
    Ok(value.map(|v| serde_json::to_string(&v).expect("Value always serializes")))
}

#[tauri::command]
#[specta::specta]
pub fn view_state_get_all(
    db: State<'_, Arc<Db>>,
) -> Result<BTreeMap<String, String>, ViewStateCommandError> {
    let all = view_state::get_all(&db)?;
    Ok(all
        .into_iter()
        .map(|(k, v)| {
            (
                k,
                serde_json::to_string(&v).expect("Value always serializes"),
            )
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
pub fn view_state_delete(db: State<'_, Arc<Db>>, key: String) -> Result<(), ViewStateCommandError> {
    view_state::delete(&db, &key).map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn view_state_reset(db: State<'_, Arc<Db>>) -> Result<(), ViewStateCommandError> {
    view_state::reset(&db).map_err(Into::into)
}
