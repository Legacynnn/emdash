//! Family A — agent-style classifiers (EMD-23).
//!
//! Each ports the Electron-side regex set verbatim. The only
//! per-agent variable is the `idle_prompt` pattern; everything else
//! is `classify_common`.

use crate::agent_hooks::classifier::ClassificationResult;
use crate::agent_hooks::text_classifier::{classify_common, BufferedTextClassifier};

fn cursor_match(text: &str) -> Option<ClassificationResult> {
    classify_common(text, r"Add a follow-up|Auto\s*[\r\n]+\s*/\s*commands")
}

fn amp_match(text: &str) -> Option<ClassificationResult> {
    classify_common(
        text,
        r"Ready|Awaiting|Press Enter|Next command|Type your message",
    )
}

fn devin_match(text: &str) -> Option<ClassificationResult> {
    classify_common(
        text,
        r"Type @|/help|/mode|/plan|/ask|What would you like|Start coding|Ready|Awaiting|Press Enter|Next command",
    )
}

fn jules_match(text: &str) -> Option<ClassificationResult> {
    classify_common(
        text,
        r"Ready|Awaiting|Press Enter|Next command|Type your message",
    )
}

fn autohand_match(text: &str) -> Option<ClassificationResult> {
    classify_common(text, r"Ready|Awaiting|Press Enter|Next command")
}

fn rovo_match(text: &str) -> Option<ClassificationResult> {
    classify_common(text, r"Ready|Awaiting|Press Enter|Next command")
}

fn opencode_match(text: &str) -> Option<ClassificationResult> {
    classify_common(text, r"Ready|Awaiting|Press Enter|Next command|>\s*$")
}

pub fn cursor_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("cursor", cursor_match)
}

pub fn amp_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("amp", amp_match)
}

pub fn devin_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("devin", devin_match)
}

pub fn jules_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("jules", jules_match)
}

pub fn autohand_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("autohand", autohand_match)
}

pub fn rovo_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("rovo", rovo_match)
}

pub fn opencode_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("opencode", opencode_match)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_hooks::event::{AgentEventKind, NotificationKind};
    use crate::agent_hooks::text_classifier::TextClassifier;

    fn assert_kind(r: Option<ClassificationResult>, expected: AgentEventKind) {
        let r = r.expect("expected a classification");
        assert_eq!(r.kind, expected);
    }

    #[test]
    fn cursor_idle_prompt_matches() {
        let c = cursor_classifier();
        assert_kind(
            c.feed("\nAdd a follow-up>"),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::IdlePrompt,
            },
        );
    }

    #[test]
    fn cursor_permission_matches() {
        let c = cursor_classifier();
        assert_kind(
            c.feed("Please approve this tool use:"),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::PermissionPrompt,
            },
        );
    }

    #[test]
    fn cursor_auth_success_matches() {
        let c = cursor_classifier();
        assert_kind(
            c.feed("Successfully authenticated as octocat"),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::AuthSuccess,
            },
        );
    }

    #[test]
    fn cursor_question_matches() {
        let c = cursor_classifier();
        assert_kind(
            c.feed("Which file should I open?"),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::ElicitationDialog,
            },
        );
    }

    #[test]
    fn cursor_error_matches() {
        let c = cursor_classifier();
        assert_kind(c.feed("fatal: bad ref"), AgentEventKind::Error);
    }

    #[test]
    fn amp_idle_matches_press_enter() {
        let c = amp_classifier();
        assert_kind(
            c.feed("\nPress Enter to continue"),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::IdlePrompt,
            },
        );
    }

    #[test]
    fn devin_idle_matches_slash_command() {
        let c = devin_classifier();
        assert_kind(
            c.feed("\n/plan"),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::IdlePrompt,
            },
        );
    }

    #[test]
    fn opencode_idle_matches_prompt_marker() {
        let c = opencode_classifier();
        assert_kind(
            c.feed("\n> "),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::IdlePrompt,
            },
        );
    }

    #[test]
    fn jules_autohand_rovo_idle_match() {
        for c in [jules_classifier(), autohand_classifier(), rovo_classifier()] {
            assert_kind(
                c.feed("\nReady for input"),
                AgentEventKind::Notification {
                    notification_kind: NotificationKind::IdlePrompt,
                },
            );
        }
    }

    #[test]
    fn no_match_returns_none() {
        let c = cursor_classifier();
        assert!(c.feed("normal log line\n").is_none());
    }

    #[test]
    fn ansi_codes_are_stripped_before_match() {
        let c = cursor_classifier();
        assert_kind(
            c.feed("\x1b[31mfatal:\x1b[0m bad ref"),
            AgentEventKind::Error,
        );
    }
}
