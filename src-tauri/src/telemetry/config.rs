//! Compile-time telemetry config.
//!
//! `EMDASH_TELEMETRY_HOST` and `EMDASH_TELEMETRY_API_KEY` are stamped
//! into the binary by `build.rs`. An empty `host` means telemetry is
//! disabled at compile time (dev builds, CI matrix, anyone without a
//! `.env` configured). The runtime treats this as identical to the
//! user toggle being off.

#[derive(Clone, Debug)]
pub struct TelemetryConfig {
    pub host: String,
    pub api_key: String,
}

impl TelemetryConfig {
    /// Read from the build-time-stamped env. Missing variables become
    /// empty strings — see module docs.
    pub fn from_build_env() -> Self {
        Self {
            host: env!("EMDASH_TELEMETRY_HOST").to_string(),
            api_key: env!("EMDASH_TELEMETRY_API_KEY").to_string(),
        }
    }

    /// Compile-time gate: an empty host means "no endpoint stamped in"
    /// and the runtime should never attempt to send.
    pub fn is_compiled_in(&self) -> bool {
        !self.host.is_empty()
    }
}
