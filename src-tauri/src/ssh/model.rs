//! Wire-format types for the SSH subsystem.
//!
//! `SshConnection` is the public representation (no secrets) that
//! the renderer sees. `NewSshConnection` carries the secret-bearing
//! fields the renderer sends in on create/update; the store layer
//! strips them before persisting and forwards them to
//! `SshCredentials`.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AuthType {
    /// User stores a password in the keychain.
    Password,
    /// User points at a key file on disk; passphrase may be in the
    /// keychain.
    Key,
    /// Delegate to ssh-agent.
    Agent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SshConnection {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: AuthType,
    /// Path to the private key file. Tilde is *not* expanded here;
    /// the renderer shows what the user typed.
    pub private_key_path: Option<String>,
    /// True when the user opted into ssh-agent (mutually exclusive
    /// with `auth_type == AuthType::Agent` only in older configs;
    /// kept for backward compat with the Electron schema).
    pub use_agent: bool,
    /// Default worktrees directory on the remote host (e.g.
    /// `/home/me/emdash-worktrees`). Stored in `metadata` JSON.
    pub worktrees_dir: Option<String>,
    /// ISO-8601 timestamp. String at the wire boundary because
    /// specta is finicky about chrono crossings without an extra
    /// feature flag.
    pub updated_at: String,
}

/// Payload for `save_connection`. The two secret fields are
/// **never** persisted in the DB row — they go through
/// `SshCredentials::store_*` and are dropped from the model
/// immediately after.
#[derive(Clone, Debug, Deserialize, Type)]
pub struct NewSshConnection {
    /// If present, the call updates the existing row; otherwise the
    /// store mints a fresh UUID.
    pub id: Option<String>,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: AuthType,
    pub private_key_path: Option<String>,
    pub use_agent: bool,
    pub worktrees_dir: Option<String>,
    /// Optional. Stored encrypted via `SshCredentials::store_password`.
    pub password: Option<String>,
    /// Optional. Stored encrypted via `SshCredentials::store_passphrase`.
    pub passphrase: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConnectionState {
    /// No live session; user explicitly disconnected or never connected.
    Disconnected,
    /// Currently negotiating.
    Connecting,
    /// Live session held in `SshConnectionManager`.
    Connected {
        /// Connection latency from the last successful test, in ms.
        latency_ms: Option<u32>,
    },
    /// Last connect attempt failed; not retrying automatically.
    Failed { error: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct ConnectionTestResult {
    pub success: bool,
    /// Round-trip time to first ssh-ready, in ms. `None` on failure.
    pub latency_ms: Option<u32>,
    pub error: Option<String>,
}
