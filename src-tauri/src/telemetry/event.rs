//! Wire-format types for telemetry events.
//!
//! Shape mirrors the Electron emdash pipeline so the receiving end
//! (currently PostHog-compatible) needs no changes when the new
//! product opts in. `event` is the canonical name; `properties` is
//! the open bag the receiver indexes on. Common props (platform,
//! arch, app_version) ride along on every envelope.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::BTreeMap;

/// Top-level event identifier. New variants are additive — receivers
/// ignore unknown names so a stale collector won't drop a release.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryEvent {
    AppFocus,
    AppUnfocus,
    AppDauPing,
    UserIdentify,
}

impl TelemetryEvent {
    pub fn name(&self) -> &'static str {
        match self {
            Self::AppFocus => "app.focus",
            Self::AppUnfocus => "app.unfocus",
            Self::AppDauPing => "app.dau_ping",
            Self::UserIdentify => "user.identify",
        }
    }
}

pub type TelemetryProps = BTreeMap<String, serde_json::Value>;

/// One outgoing event. Serialized as the body of the HTTP POST.
#[derive(Clone, Debug, Serialize)]
pub struct TelemetryEnvelope {
    pub event: String,
    pub distinct_id: String,
    pub timestamp: DateTime<Utc>,
    pub properties: TelemetryProps,
}
