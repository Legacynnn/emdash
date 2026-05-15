//! Wire-format types for file-system watch events.

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WatchEventKind {
    Created,
    Modified,
    Deleted,
    Renamed { from: String, to: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct WatchEvent {
    /// Coalesced batch of paths; for `Renamed`, the `from` / `to`
    /// inside the kind carry the pair, and `paths` is empty.
    pub paths: Vec<String>,
    pub event: WatchEventKind,
}

/// Whether the watcher is running with full recursive coverage or
/// degraded to the EMD-11 ENOSPC fallback (top-level only).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum WatcherFallback {
    None,
    /// Linux inotify limit hit; scoped to depth 1. User can remediate
    /// with `sysctl fs.inotify.max_user_watches=524288`.
    InotifyEnospcDepthOne,
}

#[derive(Debug, Error)]
pub enum WatcherError {
    #[error("path does not exist: {0}")]
    PathMissing(String),
    #[error("notify error: {0}")]
    Notify(#[from] notify::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
