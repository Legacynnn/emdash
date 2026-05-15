//! GitHub provider (EMD-13 / ADR-0022).
//!
//! Two auth paths share the same downstream client builder:
//! - **Device flow** (`oauth2`): start → poll → store token via the
//!   AEAD `app_secrets` layer (EMD-6).
//! - **`gh` CLI fallback**: opt-in shell-out to `gh auth token`.
//!
//! All token reads/writes go through `Secrets`. Identity is a plain
//! `app_settings` row (login / id / avatar / email) so the renderer
//! can render the signed-in surface without re-decrypting on every
//! mount.

pub mod auth;
pub mod client;
pub mod identity;
pub mod model;

pub use auth::{
    device_flow_poll, device_flow_start, gh_cli_token, sign_in_with_token, sign_out, DeviceFlow,
    DeviceFlowError, DeviceFlowStart, OauthScope,
};
pub use client::{client_for_token, GithubError};
pub use identity::{get_identity, IdentityRecord};
pub use model::{PullRequestSummary, RepoSummary, ViewerProfile};

/// Stored secrets keys for the GitHub token. Re-exported so the
/// Tauri glue + tests reference one constant.
pub const TOKEN_SECRET_KEY: &str = "github.access_token";
