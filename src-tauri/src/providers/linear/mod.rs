//! Linear provider (EMD-14 / ADR-0023). Mirrors the EMD-13 GitHub
//! provider shape: AEAD-only token storage, identity split into a
//! plain `app_settings` row, typed errors at the wire boundary.
//!
//! v1 ships **API-key paste only**. OAuth is explicit v1.x; the
//! `sign_in` command takes a user-pasted token straight to
//! `Secrets::set(\"linear.access_token\", ...)`.

pub mod client;
pub mod error;
pub mod identity;
pub mod model;

pub use client::LinearClient;
pub use error::LinearError;
pub use identity::{clear_identity, get_identity, set_identity, LinearIdentityRecord};
pub use model::{
    LinearCycle, LinearIssue, LinearIssueComment, LinearIssueCreateInput, LinearIssueFilter,
    LinearIssueUpdateInput, LinearLabel, LinearProject, LinearTeam, LinearViewerProfile,
    LinearWorkflowState,
};

/// AEAD `app_secrets` key under which the Linear API token lives.
pub const TOKEN_SECRET_KEY: &str = "linear.access_token";
