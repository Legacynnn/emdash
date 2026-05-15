//! Tauri glue for the updater state machine.
//!
//! The renderer subscribes to `UpdateEvent`s through a dedicated
//! `Channel<UpdateEvent>` opened by `subscribe_updater_events`, mirroring
//! the EMD-7 UiMutationEvent bridge pattern. Discrete commands drive the
//! state machine (`updater_check`, `updater_download_and_install`,
//! `updater_simulate_event` — the last one is `#[cfg(debug_assertions)]`
//! so the dev "Check for updates" button can exercise every state without
//! a real manifest).

use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use tauri::{ipc::Channel, State};

use crate::updater::{CheckReason, EventListener, UpdateError, UpdateEvent, UpdateManager};

#[derive(Debug, Serialize, Type)]
pub struct UpdaterCommandError {
    pub code: UpdaterErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum UpdaterErrorCode {
    AlreadyChecking,
    Internal,
}

#[tauri::command]
#[specta::specta]
pub async fn subscribe_updater_events(
    manager: State<'_, Arc<UpdateManager>>,
    on_event: Channel<UpdateEvent>,
) -> Result<(), ()> {
    let listener: EventListener = Arc::new(move |event| {
        // Fire-and-forget — same loss-tolerance contract as PTY +
        // UiMutationEvent (ADR-0003 / ADR-0004).
        let _ = on_event.send(event);
    });
    manager.on_event(listener);
    Ok(())
}

/// Starts a check. The real `tauri-plugin-updater::check()` call lives
/// behind this command — we wrap it so the state machine sees the
/// transitions before the plugin's own events fire. EMD-22 wires the
/// real plugin path; for now we just begin and let the caller drive
/// transitions through `updater_simulate_event` in dev builds.
#[tauri::command]
#[specta::specta]
pub async fn updater_check(
    manager: State<'_, Arc<UpdateManager>>,
    reason: CheckReason,
) -> Result<(), UpdaterCommandError> {
    manager.begin_check(reason);
    Ok(())
}

/// Debug-only: hand the renderer a trigger to walk the state machine
/// through every UpdateEvent variant without a real update manifest.
/// Production builds drop this command entirely.
#[cfg(debug_assertions)]
#[tauri::command]
#[specta::specta]
pub async fn updater_simulate_event(
    manager: State<'_, Arc<UpdateManager>>,
    event: UpdateEvent,
) -> Result<(), UpdaterCommandError> {
    match event {
        UpdateEvent::Checking { reason } => manager.begin_check(reason),
        UpdateEvent::UpToDate => manager.mark_up_to_date(),
        UpdateEvent::Available { version, notes } => manager.mark_available(version, notes),
        UpdateEvent::Downloading { progress } => manager.record_progress(progress),
        UpdateEvent::ReadyToInstall { version } => manager.mark_ready_to_install(version),
        UpdateEvent::Error {
            code,
            message,
            will_retry: _,
        } => {
            // Use Manual so the simulated failure doesn't kick off a
            // real backoff retry loop on the dev box.
            let _ = manager.mark_failed(code, message, CheckReason::Manual);
        }
    }
    Ok(())
}

/// Stub for production builds: the dev simulator is the only way to
/// drive the state machine until EMD-22 wires real `tauri-plugin-updater`
/// interaction. Once that lands this command goes away.
#[cfg(not(debug_assertions))]
#[tauri::command]
#[specta::specta]
pub async fn updater_simulate_event(
    _manager: State<'_, Arc<UpdateManager>>,
    _event: UpdateEvent,
) -> Result<(), UpdaterCommandError> {
    Err(UpdaterCommandError {
        code: UpdaterErrorCode::Internal,
        message: "updater_simulate_event is debug-only".into(),
    })
}

/// Sentinel for the lint that ensures every `UpdateError` variant
/// has a renderer string. The variant set lives in the domain layer;
/// `_` patterns are forbidden so adding a variant fails compilation
/// until it gets a UI string.
pub fn _exhaustiveness_check(err: UpdateError) {
    match err {
        UpdateError::Network
        | UpdateError::SignatureInvalid
        | UpdateError::ManifestMalformed
        | UpdateError::DownloadFailed
        | UpdateError::InstallFailed
        | UpdateError::Internal => {}
    }
}
