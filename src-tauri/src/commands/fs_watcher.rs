//! Tauri glue for the `fs_watcher` domain (EMD-11 / ADR-0020).
//!
//! `subscribe_fs_watcher(id, path, channel)` opens a `WatchEvent`
//! channel for the given path. The renderer reads `fallback` from
//! the return to decide whether to surface the inotify-ENOSPC toast.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use tauri::{ipc::Channel, State};

use crate::fs_watcher::{
    registry::EventListener, WatchEvent, WatcherError, WatcherFallback, WatcherRegistry,
};

#[derive(Debug, Serialize, Type)]
pub struct FsWatcherCommandError {
    pub code: FsWatcherErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum FsWatcherErrorCode {
    PathMissing,
    Notify,
    Io,
}

impl From<WatcherError> for FsWatcherCommandError {
    fn from(e: WatcherError) -> Self {
        let code = match &e {
            WatcherError::PathMissing(_) => FsWatcherErrorCode::PathMissing,
            WatcherError::Notify(_) => FsWatcherErrorCode::Notify,
            WatcherError::Io(_) => FsWatcherErrorCode::Io,
        };
        let message = e.to_string();
        Self { code, message }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn fs_watcher_subscribe(
    registry: State<'_, Arc<WatcherRegistry>>,
    id: String,
    path: String,
    on_event: Channel<WatchEvent>,
) -> Result<WatcherFallback, FsWatcherCommandError> {
    let listener: EventListener = Arc::new(move |event| {
        let _ = on_event.send(event);
    });
    registry
        .watch(id, PathBuf::from(path), listener)
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn fs_watcher_unsubscribe(
    registry: State<'_, Arc<WatcherRegistry>>,
    id: String,
) -> Result<bool, FsWatcherCommandError> {
    Ok(registry.unwatch(&id))
}
