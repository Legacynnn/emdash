//! `UiMutationEvent` — the single typed enum carrying renderer cache-
//! invalidation signals. Stable JSON wire format pinned by `insta`
//! snapshots in `tests/wire_format.rs`.
//!
//! `serde(tag = "kind")` is load-bearing: drop it and the renderer match
//! over variants breaks silently.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiMutationEvent {
    ProjectCreated {
        id: String,
    },
    ProjectUpdated {
        id: String,
    },
    ProjectDeleted {
        id: String,
    },
    /// EMD-13: GitHub identity changed (sign-in / sign-out / refresh).
    GithubIdentityChanged,
    /// EMD-13: GitHub repo-scoped data changed (PRs, comments, reviews
    /// of the named `owner/name`).
    GithubDataChanged {
        repo: String,
    },
}
