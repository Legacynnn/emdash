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
    WorkspaceCreated {
        id: String,
        project_id: String,
    },
    WorkspaceUpdated {
        id: String,
        project_id: String,
    },
    WorkspaceDeleted {
        id: String,
        project_id: String,
    },
    /// One classified agent-hook event (EMD-9). `conversation_id` is
    /// the primary routing key now that agents are per-conversation;
    /// `workspace_id` is retained for the renderer's fan-out fallback
    /// when the hook command didn't propagate `EMDASH_CONVERSATION_ID`.
    AgentHookEvent {
        conversation_id: Option<String>,
        workspace_id: Option<String>,
        event: crate::agent_hooks::AgentEvent,
    },
    /// EMD-13: GitHub identity changed (sign-in / sign-out / refresh).
    GithubIdentityChanged,
    /// EMD-13: GitHub repo-scoped data changed (PRs, comments, reviews
    /// of the named `owner/name`).
    GithubDataChanged {
        repo: String,
    },
    /// EMD-14: Linear identity changed (sign-in / sign-out / refresh).
    LinearIdentityChanged,
    /// EMD-14: Linear team-scoped data changed (issues, comments).
    LinearDataChanged {
        team: String,
    },
    /// EMD-27: an agent started for the named conversation. Per-
    /// conversation routing (each conversation tab owns its own agent
    /// process) replaces the earlier one-per-workspace constraint.
    AgentStarted {
        conversation_id: String,
        provider: crate::agents::AgentProvider,
    },
    /// EMD-27: an agent exited (clean or killed). `exit_code` is
    /// `None` when the host stopped the agent before the child
    /// process reported a code.
    AgentExited {
        conversation_id: String,
        exit_code: Option<i32>,
    },
    /// A conversation was created under a workspace.
    ConversationCreated {
        id: String,
        workspace_id: String,
        project_id: String,
    },
    /// A conversation was renamed or had its recency bumped.
    ConversationUpdated {
        id: String,
        workspace_id: String,
        project_id: String,
    },
    ConversationDeleted {
        id: String,
        workspace_id: String,
        project_id: String,
    },
    /// Terminal tabs are siblings of conversations under a workspace.
    TerminalCreated {
        id: String,
        workspace_id: String,
        project_id: String,
    },
    TerminalUpdated {
        id: String,
        workspace_id: String,
        project_id: String,
    },
    TerminalDeleted {
        id: String,
        workspace_id: String,
        project_id: String,
    },
    /// A skill was installed (catalog state flipped).
    SkillInstalled {
        id: String,
    },
    /// A skill was uninstalled.
    SkillUninstalled {
        id: String,
    },
    /// The skills catalog was refreshed from upstream / disk.
    SkillsCatalogRefreshed,
    /// An MCP server config was created or updated.
    McpServerSaved {
        name: String,
    },
    /// An MCP server config was removed across all targeted agents.
    McpServerRemoved {
        name: String,
    },
    /// The MCP provider-installed snapshot was re-probed.
    McpProvidersRefreshed,
}
