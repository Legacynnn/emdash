//! Auto-updater state machine (EMD-15).
//!
//! Wraps `tauri-plugin-updater` v2 in a Helmor-style `UpdateManager`
//! that owns the state, throttles progress events, applies an
//! exponential backoff on retries, and installs pending updates on
//! `RunEvent::Exit`. The full rationale lives in ADR-0008.
//!
//! Domain-side stays Tauri-runtime-free; the actual plugin glue lives
//! in `commands::updater` and `app.rs` (the latter wires the plugin
//! and the exit hook).

pub mod backoff;
pub mod event;
pub mod manifest;
pub mod state;

pub use backoff::{Backoff, BackoffConfig};
pub use event::{CheckReason, UpdateError, UpdateEvent};
pub use manifest::{ManifestPlatform, UpdateManifest, LATEST_JSON_SCHEMA_DOC};
pub use state::{EventListener, UpdateManager, UpdateState};
