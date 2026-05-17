//! Wire-format envelope for agent-hook events.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Renderer-visible projection of one classified hook event.
/// `conversation_id` / `workspace_id` / `project_id` are filled in
/// by the agent-spawn site (EMD-27) — agents are per-conversation
/// now, so the conversation id is the primary routing key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AgentEvent {
    pub agent: String,
    pub classifier: String,
    pub kind: AgentEventKind,
    pub message: Option<String>,
    /// ISO-8601 UTC timestamp set at enrichment time. Stored as a
    /// string at the wire boundary because specta requires a
    /// dedicated feature flag to round-trip `chrono::DateTime`.
    pub timestamp: String,
    /// Primary routing key. `None` for hook events received before
    /// the renderer's user-configured hook command propagates
    /// `EMDASH_CONVERSATION_ID` into the POST body.
    pub conversation_id: Option<String>,
    pub workspace_id: Option<String>,
    pub project_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentEventKind {
    /// A normal in-flight notification (e.g. tool use, status message).
    Notification {
        #[serde(default)]
        notification_kind: NotificationKind,
    },
    /// Agent's session ended cleanly.
    Stop,
    /// Agent reported an error.
    Error,
    /// Classifier didn't recognize the input; surfaces as an opaque
    /// pass-through so the renderer can still log it.
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    #[default]
    General,
    PermissionPrompt,
    ToolUse,
    Status,
    /// EMD-23+: agent is idle / awaiting input (e.g. ready prompt).
    IdlePrompt,
    /// EMD-23+: agent sign-in / OAuth flow succeeded.
    AuthSuccess,
    /// EMD-23+: agent asks the user a clarifying question.
    ElicitationDialog,
}
