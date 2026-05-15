//! Wire-format types for the updater state machine. The renderer
//! receives one `UpdateEvent` per state transition and one
//! `Downloading { progress }` per ~200 ms tick during the download
//! phase (see [`UpdateManager`] for the throttle).

use serde::{Deserialize, Serialize};
use specta::Type;

/// Why the manager kicked off a check. Used both for backoff bucket
/// selection (a `Manual` check should retry faster than a passive
/// `Interval` check) and for renderer-side UX (a `Manual` failure
/// surfaces a louder error message).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CheckReason {
    Startup,
    Resume,
    Focus,
    Interval,
    Manual,
}

/// State the renderer needs to render the updater UI. The variants
/// are intentionally narrow — internal state machine transitions
/// that don't change what the user sees never broadcast.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UpdateEvent {
    /// A check has started. The renderer shows a spinner.
    Checking { reason: CheckReason },
    /// The server confirms we're on the latest version.
    UpToDate,
    /// A newer version is available. Renderer shows "Download".
    Available {
        version: String,
        notes: Option<String>,
    },
    /// Download progress. The throttle caps these at ~200 ms apart
    /// so a fast download doesn't flood the renderer with re-renders.
    Downloading { progress: f64 },
    /// Download complete, pending install on exit. Renderer can
    /// surface "Restart to install" if it wants to fast-path the
    /// `RunEvent::Exit` hook.
    ReadyToInstall { version: String },
    /// A check or download failed. The manager will retry per the
    /// backoff schedule unless the reason was `Manual`.
    Error {
        code: UpdateError,
        message: String,
        will_retry: bool,
    },
}

/// Stable machine-readable error code. The renderer matches on this
/// to decide whether to suggest "Retry now" (network) or "Reinstall"
/// (signature_invalid).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum UpdateError {
    Network,
    SignatureInvalid,
    ManifestMalformed,
    DownloadFailed,
    InstallFailed,
    Internal,
}
