//! Family B — IDE-style classifiers (EMD-24).
//!
//! Compared with family A, these vary more across agents:
//!
//! - Several emit a `stop` signal on task-completion markers
//!   (cline, kilocode).
//! - Some have several disjoint idle-prompt regexes that don't OR
//!   into one pattern cleanly (continue, kilocode).
//! - Some use bespoke permission patterns (copilot, junie).
//! - `gemini` is minimal — only permission + idle, nothing else.
//!
//! Each one is therefore a `ClassifySpec` literal rather than a
//! one-arg call to `classify_common`. The shared `run_spec` walks
//! the same ordered six-check set.

use crate::agent_hooks::classifier::ClassificationResult;
use crate::agent_hooks::text_classifier::{run_spec, BufferedTextClassifier, ClassifySpec};

// --- Specs ----------------------------------------------------------------

const PERM_DEFAULT: &str = r"approve|reject|permission|allow|confirm";
const AUTH_DEFAULT: &str = r"Successfully authenticated|Login successful";
const ELICIT_DEFAULT: &str = r"What.*\?|How.*\?|Which.*\?|Please (provide|specify|clarify)";
const ERROR_DEFAULT: &str = r"error:|fatal:|exception|failed";

const CLINE_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(PERM_DEFAULT),
    stop: Some((r"Task completed|Done\.", "Task completed")),
    idle: &[r"Ready|Awaiting|Press Enter|Next command", r"cline\s*>"],
    auth: Some(AUTH_DEFAULT),
    elicitation: Some(r"What.*\?|How can I|Please.*:"),
    error: Some(ERROR_DEFAULT),
};

const CONTINUE_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(PERM_DEFAULT),
    stop: None,
    idle: &[r"Ready|Awaiting|Press Enter", r"cn\s*>|continue\s*>"],
    auth: Some(AUTH_DEFAULT),
    elicitation: Some(r"What.*\?|How can I|Next step"),
    error: Some(ERROR_DEFAULT),
};

const COPILOT_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(
        r"Do you want to|Confirm with number keys|approve all file operations|Yes, and approve",
    ),
    stop: None,
    idle: &[r"Ready|Press Enter|Next step"],
    auth: Some(AUTH_DEFAULT),
    elicitation: Some(ELICIT_DEFAULT),
    error: Some(ERROR_DEFAULT),
};

const DROID_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(PERM_DEFAULT),
    stop: None,
    idle: &[r"Ready|Awaiting|Press Enter"],
    auth: Some(r"Successfully authenticated|Login successful|API key valid"),
    elicitation: Some(ELICIT_DEFAULT),
    error: Some(ERROR_DEFAULT),
};

const GEMINI_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(r"Action Required"),
    stop: None,
    idle: &[r"\[INSERT\]|\[NORMAL\]"],
    auth: None,
    elicitation: None,
    error: None,
};

const JUNIE_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(r"approve|reject|permission|allow|confirm|brave mode"),
    stop: None,
    idle: &[r"Ready|Awaiting|Press Enter|Next command|Junie"],
    auth: Some(r"Successfully authenticated|Login successful|authenticated with Junie"),
    elicitation: Some(ELICIT_DEFAULT),
    error: Some(ERROR_DEFAULT),
};

const KILOCODE_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(PERM_DEFAULT),
    stop: Some((r"✓\s*Task Completed|Checkpoint Saved", "Task completed")),
    idle: &[
        r"Type a message or /command",
        r"What would you like to work on",
        r"Ready|Awaiting|Press Enter|Next command",
        r"/help for commands|/mode to switch mode|! for shell mode",
    ],
    auth: Some(AUTH_DEFAULT),
    elicitation: Some(ELICIT_DEFAULT),
    error: Some(ERROR_DEFAULT),
};

const KIRO_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(PERM_DEFAULT),
    stop: None,
    idle: &[r"Ready|Awaiting|Press Enter|Next command|Kiro CLI"],
    auth: Some(AUTH_DEFAULT),
    elicitation: Some(ELICIT_DEFAULT),
    error: Some(ERROR_DEFAULT),
};

// --- Matchers + factories -------------------------------------------------

fn cline_match(t: &str) -> Option<ClassificationResult> {
    run_spec(t, &CLINE_SPEC)
}
fn continue_match(t: &str) -> Option<ClassificationResult> {
    run_spec(t, &CONTINUE_SPEC)
}
fn copilot_match(t: &str) -> Option<ClassificationResult> {
    run_spec(t, &COPILOT_SPEC)
}
fn droid_match(t: &str) -> Option<ClassificationResult> {
    run_spec(t, &DROID_SPEC)
}
fn gemini_match(t: &str) -> Option<ClassificationResult> {
    run_spec(t, &GEMINI_SPEC)
}
fn junie_match(t: &str) -> Option<ClassificationResult> {
    run_spec(t, &JUNIE_SPEC)
}
fn kilocode_match(t: &str) -> Option<ClassificationResult> {
    run_spec(t, &KILOCODE_SPEC)
}
fn kiro_match(t: &str) -> Option<ClassificationResult> {
    run_spec(t, &KIRO_SPEC)
}

pub fn cline_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("cline", cline_match)
}
pub fn continue_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("continue", continue_match)
}
pub fn copilot_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("copilot", copilot_match)
}
pub fn droid_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("droid", droid_match)
}
pub fn gemini_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("gemini", gemini_match)
}
pub fn junie_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("junie", junie_match)
}
pub fn kilocode_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("kilocode", kilocode_match)
}
pub fn kiro_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("kiro", kiro_match)
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
    fn cline_emits_stop_on_task_completed() {
        let c = cline_classifier();
        let r = c.feed("\nTask completed").unwrap();
        assert_eq!(r.kind, AgentEventKind::Stop);
        assert_eq!(r.message.as_deref(), Some("Task completed"));
    }

    #[test]
    fn cline_emits_idle_on_prompt_marker() {
        let c = cline_classifier();
        assert_kind(
            c.feed("\ncline> "),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::IdlePrompt,
            },
        );
    }

    #[test]
    fn continue_emits_idle_on_short_prompt() {
        let c = continue_classifier();
        assert_kind(
            c.feed("\ncn > "),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::IdlePrompt,
            },
        );
    }

    #[test]
    fn copilot_uses_bespoke_permission_pattern() {
        let c = copilot_classifier();
        assert_kind(
            c.feed("Do you want to apply these changes?"),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::PermissionPrompt,
            },
        );
    }

    #[test]
    fn droid_recognizes_api_key_valid_as_auth() {
        let c = droid_classifier();
        assert_kind(
            c.feed("API key valid"),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::AuthSuccess,
            },
        );
    }

    #[test]
    fn gemini_is_minimal_no_error_path() {
        let c = gemini_classifier();
        // gemini has no error pattern; this should fall through to None.
        assert!(c.feed("fatal: bad ref").is_none());
        assert_kind(
            c.feed("\n[INSERT]"),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::IdlePrompt,
            },
        );
        assert_kind(
            c.feed("Action Required"),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::PermissionPrompt,
            },
        );
    }

    #[test]
    fn junie_recognizes_brave_mode_as_permission() {
        let c = junie_classifier();
        assert_kind(
            c.feed("brave mode active"),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::PermissionPrompt,
            },
        );
    }

    #[test]
    fn kilocode_stop_on_checkpoint_saved() {
        let c = kilocode_classifier();
        let r = c.feed("Checkpoint Saved").unwrap();
        assert_eq!(r.kind, AgentEventKind::Stop);
    }

    #[test]
    fn kilocode_idle_on_help_text() {
        let c = kilocode_classifier();
        assert_kind(
            c.feed("/help for commands"),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::IdlePrompt,
            },
        );
    }

    #[test]
    fn kiro_recognizes_kiro_cli_as_idle() {
        let c = kiro_classifier();
        assert_kind(
            c.feed("Kiro CLI ready"),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::IdlePrompt,
            },
        );
    }

    #[test]
    fn all_idle_patterns_match_default_ready() {
        for c in [
            cline_classifier(),
            continue_classifier(),
            copilot_classifier(),
            droid_classifier(),
            junie_classifier(),
            kilocode_classifier(),
            kiro_classifier(),
        ] {
            let r = c.feed("\nReady for input").unwrap_or_else(|| {
                panic!("{} did not match a default idle prompt", c.name());
            });
            assert!(matches!(
                r.kind,
                AgentEventKind::Notification {
                    notification_kind: NotificationKind::IdlePrompt
                }
            ));
        }
    }
}
