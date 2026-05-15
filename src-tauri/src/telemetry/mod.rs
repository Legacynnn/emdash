//! Telemetry pipeline.
//!
//! Defaults off in v1 — see ADR-0007. Compile-time `EMDASH_TELEMETRY_HOST`
//! and `EMDASH_TELEMETRY_API_KEY` (injected by `build.rs`) gate whether
//! the pipeline can run *at all*; a user toggle stored in `app_settings`
//! gates whether it *does* run within that. Both must be non-empty /
//! `true` for an event to leave the machine.

pub mod config;
pub mod event;
pub mod runtime;
pub mod settings;

pub use config::TelemetryConfig;
pub use event::{TelemetryEnvelope, TelemetryEvent, TelemetryProps};
pub use runtime::{Telemetry, TelemetryError};
pub use settings::{get_enabled, set_enabled, SETTINGS_KEY};
