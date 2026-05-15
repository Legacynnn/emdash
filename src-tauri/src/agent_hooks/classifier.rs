//! `Classifier` trait + claude reference implementation.
//!
//! Each classifier takes the JSON body posted to `/hook` and decides
//! whether (and how) to convert it into a renderer-visible event.
//! Returning `Unknown` is fine; the server still broadcasts so the
//! event isn't lost.
//!
//! claude is the reference impl. The remaining 28 classifiers live
//! in their family-specific issues (EMD-23/24/25/26).

use serde::Deserialize;

use super::event::{AgentEventKind, NotificationKind};

/// What the classifier emits when handed an event body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassificationResult {
    pub kind: AgentEventKind,
    pub message: Option<String>,
}

/// Implement once per agent. The classifier name registered with the
/// `ClassifierRegistry` (e.g. `"claude"`) is what the hook body's
/// `agent` field must match for routing to land here.
pub trait Classifier: Send + Sync {
    fn name(&self) -> &'static str;
    fn classify(&self, body: &serde_json::Value) -> ClassificationResult;
}

// --- claude reference implementation ---------------------------------------

/// Claude Code's hook event payload (the bits we care about). The
/// schema is documented at
/// <https://docs.claude.com/en/docs/claude-code/hooks>; this is a
/// permissive subset.
#[derive(Debug, Deserialize)]
struct ClaudePayload {
    /// `PreToolUse`, `PostToolUse`, `Stop`, `Notification`, ...
    #[serde(default)]
    hook_event_name: Option<String>,
    /// Free-text status message that Claude attaches to some events.
    #[serde(default)]
    message: Option<String>,
    /// On `PreToolUse` / `PostToolUse`, the tool's name.
    #[serde(default)]
    tool_name: Option<String>,
    /// On `Stop`, optional reason.
    #[serde(default)]
    reason: Option<String>,
}

pub struct ClaudeClassifier;

impl Classifier for ClaudeClassifier {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn classify(&self, body: &serde_json::Value) -> ClassificationResult {
        let payload: ClaudePayload = match serde_json::from_value(body.clone()) {
            Ok(p) => p,
            // Malformed payloads still broadcast as Unknown so the
            // renderer can show them; nothing is silently dropped.
            Err(_) => {
                return ClassificationResult {
                    kind: AgentEventKind::Unknown,
                    message: None,
                };
            }
        };
        match payload.hook_event_name.as_deref() {
            Some("Stop") => ClassificationResult {
                kind: AgentEventKind::Stop,
                message: payload.reason.or(payload.message),
            },
            Some("Error") => ClassificationResult {
                kind: AgentEventKind::Error,
                message: payload.message,
            },
            Some("Notification") => ClassificationResult {
                kind: AgentEventKind::Notification {
                    notification_kind: NotificationKind::General,
                },
                message: payload.message,
            },
            Some("PreToolUse") | Some("PostToolUse") => ClassificationResult {
                kind: AgentEventKind::Notification {
                    notification_kind: NotificationKind::ToolUse,
                },
                message: payload.tool_name.map(|t| format!("tool: {t}")),
            },
            _ => ClassificationResult {
                kind: AgentEventKind::Unknown,
                message: payload.message,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn classifies_stop_event() {
        let r = ClaudeClassifier.classify(&json!({
            "hook_event_name": "Stop",
            "reason": "session ended"
        }));
        assert_eq!(r.kind, AgentEventKind::Stop);
        assert_eq!(r.message.as_deref(), Some("session ended"));
    }

    #[test]
    fn classifies_pretooluse_as_tool_use_notification() {
        let r = ClaudeClassifier.classify(&json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash"
        }));
        assert!(matches!(
            r.kind,
            AgentEventKind::Notification {
                notification_kind: NotificationKind::ToolUse
            }
        ));
        assert_eq!(r.message.as_deref(), Some("tool: Bash"));
    }

    #[test]
    fn classifies_notification_event() {
        let r = ClaudeClassifier.classify(&json!({
            "hook_event_name": "Notification",
            "message": "permission needed"
        }));
        assert!(matches!(
            r.kind,
            AgentEventKind::Notification {
                notification_kind: NotificationKind::General
            }
        ));
        assert_eq!(r.message.as_deref(), Some("permission needed"));
    }

    #[test]
    fn unknown_event_yields_unknown_kind() {
        let r = ClaudeClassifier.classify(&json!({
            "hook_event_name": "WhatIsThis",
            "message": "??"
        }));
        assert_eq!(r.kind, AgentEventKind::Unknown);
        assert_eq!(r.message.as_deref(), Some("??"));
    }

    #[test]
    fn malformed_body_is_unknown_not_panic() {
        let r = ClaudeClassifier.classify(&json!("not an object"));
        assert_eq!(r.kind, AgentEventKind::Unknown);
        assert!(r.message.is_none());
    }
}
