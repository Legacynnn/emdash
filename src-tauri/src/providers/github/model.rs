//! Renderer-facing projections. We deliberately keep these narrow
//! and re-derive `Type` so the wire format is stable across
//! `octocrab` upgrades.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct ViewerProfile {
    pub login: String,
    /// String form because GitHub user IDs are 64-bit and specta
    /// rejects i64 over IPC (see EMD-21's editor_buffer note).
    pub id: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct RepoSummary {
    pub owner: String,
    pub name: String,
    pub full_name: String,
    pub private: bool,
    pub default_branch: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct PullRequestSummary {
    pub number: i32,
    pub title: String,
    pub state: String,
    pub draft: bool,
    pub html_url: String,
    pub author: Option<String>,
    pub base_ref: String,
    pub head_ref: String,
    /// ISO-8601; same string-on-the-wire choice as editor_buffers
    /// and agent_hooks.
    pub created_at: String,
    pub updated_at: String,
}
