//! SSH remote-host integration (EMD-10).
//!
//! v1 scope:
//!
//! - CRUD for saved connections (`ssh_connections` table).
//! - Per-connection password / passphrase storage via the AEAD
//!   secrets service from EMD-6 — never plaintext.
//! - `openssh`-crate-backed client: connect over control socket,
//!   run remote `exec`, disconnect.
//! - Connection manager: pool of live sessions keyed by
//!   connection id; idempotent `connect`.
//!
//! Out of scope for v1 (tracked separately):
//!
//! - **PTY over SSH** — needs portable-pty + ssh argv plumbing,
//!   coordinated with EMD-27's spawn site.
//! - **SFTP file listing** — easy follow-up; `openssh` exposes
//!   `Sftp` but the directory walking + entry sorting is its own
//!   serialization shape.
//! - **Health monitoring + auto-reconnect** — uses `getHealthStates`
//!   in TS; defer until the renderer side actually shows health.
//! - **`~/.ssh/config` parsing** — `openssh` already respects
//!   user `ssh_config` because it shells out to the system `ssh`.
//!   The TS-side parser was needed only to derive options ssh2
//!   couldn't read from disk; we get them for free.
//! - **`testConnection` debug logs** — `openssh` does not surface
//!   the wire-level ssh debug stream; if needed, we can shell to
//!   `ssh -v` separately.

pub mod client;
pub mod credentials;
pub mod error;
pub mod manager;
pub mod model;
pub mod store;

pub use client::SshClient;
pub use credentials::SshCredentials;
pub use error::SshError;
pub use manager::SshConnectionManager;
pub use model::{AuthType, ConnectionState, ConnectionTestResult, NewSshConnection, SshConnection};
pub use store::SshConnectionStore;
