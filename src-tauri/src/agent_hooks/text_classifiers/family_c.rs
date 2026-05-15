//! Family C — niche classifiers (EMD-25).
//!
//! 11 small one-offs: `auggie`, `charm`, `codebuff`, `freebuff`
//! (codebuff variant), `generic`, `goose`, `kimi`, `letta`,
//! `mistral`, `pi`, `qwen`.
//!
//! Most fit `ClassifySpec` directly. Two exceptions:
//!
//! - `mistral` has a final idle marker (`vibe>|›|»|>`) gated on
//!   `tail.length < 100` — can't be expressed in a const spec, so
//!   it uses an inline `match_fn`.
//! - `pi`'s `stop` check (JSON `"type":"agent_end"`) sits *after*
//!   `elicitation` in the TS source. We move it to the natural
//!   spec position (before `idle`); the JSON-only pattern can't
//!   collide with idle text in practice, so this is a no-op
//!   behavior change. Noted in ADR-0027.

use crate::agent_hooks::classifier::ClassificationResult;
use crate::agent_hooks::event::{AgentEventKind, NotificationKind};
use crate::agent_hooks::text_classifier::{run_spec, BufferedTextClassifier, ClassifySpec};

const PERM_DEFAULT: &str = r"approve|reject|permission|allow|confirm";
const AUTH_DEFAULT: &str = r"Successfully authenticated|Login successful";
const ELICIT_DEFAULT: &str = r"What.*\?|How.*\?|Which.*\?|Please (provide|specify|clarify)";
const ERROR_DEFAULT: &str = r"error:|fatal:|exception|failed";

const AUGGIE_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(PERM_DEFAULT),
    stop: None,
    idle: &[r"Ready|Awaiting|Press Enter|Next command", r"auggie\s*>"],
    auth: Some(AUTH_DEFAULT),
    elicitation: Some(r"What.*\?|How can I|Please provide"),
    error: Some(ERROR_DEFAULT),
};

const CHARM_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(PERM_DEFAULT),
    stop: None,
    idle: &[r"Ready|Awaiting|Press Enter", r"crush\s*>"],
    auth: Some(AUTH_DEFAULT),
    elicitation: Some(r"What.*\?|Choose|Select"),
    error: Some(ERROR_DEFAULT),
};

const CODEBUFF_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(PERM_DEFAULT),
    stop: None,
    idle: &[r"Ready|Awaiting|Press Enter", r"codebuff\s*>"],
    auth: Some(AUTH_DEFAULT),
    elicitation: Some(r"What.*\?|Enter.*command"),
    error: Some(ERROR_DEFAULT),
};

const FREEBUFF_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(PERM_DEFAULT),
    stop: None,
    idle: &[r"Ready|Awaiting|Press Enter", r"freebuff\s*>"],
    auth: Some(AUTH_DEFAULT),
    elicitation: Some(r"What.*\?|Enter.*command"),
    error: Some(ERROR_DEFAULT),
};

const GENERIC_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(r"\[y/n\]|Continue\?|approve|reject|permission|allow|confirm"),
    stop: Some((r"✓|✔|Task completed|Finished|Done\.", "Task completed")),
    idle: &[
        r"Ready|Awaiting|Press Enter|Next command",
        r"Add a follow-up",
    ],
    auth: Some(r"Successfully authenticated|Login successful|API key (accepted|valid)"),
    elicitation: Some(ELICIT_DEFAULT),
    error: Some(ERROR_DEFAULT),
};

const GOOSE_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(PERM_DEFAULT),
    stop: None,
    idle: &[
        r"Session.*started|Session.*resumed",
        r"Ready|Awaiting|Press Enter",
        r"goose\s*>",
    ],
    auth: Some(AUTH_DEFAULT),
    elicitation: Some(r"What.*\?|How can I|Choose"),
    error: Some(ERROR_DEFAULT),
};

const KIMI_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(PERM_DEFAULT),
    // Ported verbatim — the `qwen>` in kimi.ts looks like a TS
    // copy-paste, but we keep it intact: silently changing
    // patterns risks changing what notifications users see.
    idle: &[
        r"Ready|Awaiting|Press Enter|Next command|/help|/setup",
        r"qwen\s*>",
    ],
    stop: None,
    auth: Some(AUTH_DEFAULT),
    elicitation: Some(r"What.*\?|How can I|Please.*:"),
    error: Some(ERROR_DEFAULT),
};

const LETTA_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(PERM_DEFAULT),
    stop: None,
    idle: &[
        r"Press\s+Tab|/help|/connect|/model|/agents",
        r"(?m)letta\s*>|^>\s*$",
    ],
    auth: Some(r"Successfully authenticated|Successfully connected|Login successful"),
    elicitation: Some(ELICIT_DEFAULT),
    error: Some(ERROR_DEFAULT),
};

// pi: TS source orders perm → idle → auth → elicit → stop → error.
// We hoist `stop` to the spec's natural position (before idle).
// The stop pattern is JSON-only ("type":"agent_end") so it cannot
// collide with idle text — no behavior change in practice.
const PI_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(PERM_DEFAULT),
    stop: Some((r#""type"\s*:\s*"agent_end""#, "Task completed")),
    idle: &[r"Ready|Awaiting|Press Enter|Next command"],
    auth: Some(AUTH_DEFAULT),
    elicitation: Some(ELICIT_DEFAULT),
    error: Some(ERROR_DEFAULT),
};

const QWEN_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(PERM_DEFAULT),
    stop: Some((r"Task completed|Finished", "Task completed")),
    idle: &[r"Ready|Awaiting|Press Enter|Next command", r"qwen\s*>"],
    auth: Some(AUTH_DEFAULT),
    elicitation: Some(r"What.*\?|How can I|Please.*:"),
    error: Some(ERROR_DEFAULT),
};

// --- Mistral has a tail-length-constrained idle pattern -------------------

const MISTRAL_PRE_SPEC: ClassifySpec = ClassifySpec {
    permission: Some(r"\[y/n\]|Continue\?|Approve|Reject|Cancel"),
    stop: Some((
        r"✓|✔|Completed|Finished|Done\.|Task completed",
        "Task completed",
    )),
    idle: &[
        r"Type.*message|Enter.*prompt",
        r"What would you like|How can I help",
        r"Ready|Awaiting|Press Enter|Next command",
    ],
    // auth/elicit/error are checked *after* the length-gated idle below.
    auth: None,
    elicitation: None,
    error: None,
};

const MISTRAL_POST_SPEC: ClassifySpec = ClassifySpec {
    permission: None,
    stop: None,
    idle: &[],
    auth: Some(r"Successfully authenticated|Login successful|API key accepted"),
    elicitation: Some(r"What.*\?|Please.*:"),
    error: Some(ERROR_DEFAULT),
};

fn mistral_match(text: &str) -> Option<ClassificationResult> {
    if let Some(r) = run_spec(text, &MISTRAL_PRE_SPEC) {
        return Some(r);
    }
    let tail_start = text.len().saturating_sub(500);
    let tail = &text[tail_start..];
    // Length-gated last-resort idle marker: short tail containing a
    // visible prompt sigil. Without the length gate this would over-
    // fire on any output that ends with `>`.
    if tail.len() < 100 {
        use regex::RegexBuilder;
        if let Ok(re) = RegexBuilder::new(r"\bvibe\s*>|›|»|>")
            .case_insensitive(true)
            .build()
        {
            if re.is_match(tail) {
                return Some(ClassificationResult {
                    kind: AgentEventKind::Notification {
                        notification_kind: NotificationKind::IdlePrompt,
                    },
                    message: None,
                });
            }
        }
    }
    run_spec(text, &MISTRAL_POST_SPEC)
}

// --- Matchers + factories -------------------------------------------------

fn auggie_match(t: &str) -> Option<ClassificationResult> {
    run_spec(t, &AUGGIE_SPEC)
}
fn charm_match(t: &str) -> Option<ClassificationResult> {
    run_spec(t, &CHARM_SPEC)
}
fn codebuff_match(t: &str) -> Option<ClassificationResult> {
    run_spec(t, &CODEBUFF_SPEC)
}
fn freebuff_match(t: &str) -> Option<ClassificationResult> {
    run_spec(t, &FREEBUFF_SPEC)
}
fn generic_match(t: &str) -> Option<ClassificationResult> {
    run_spec(t, &GENERIC_SPEC)
}
fn goose_match(t: &str) -> Option<ClassificationResult> {
    run_spec(t, &GOOSE_SPEC)
}
fn kimi_match(t: &str) -> Option<ClassificationResult> {
    run_spec(t, &KIMI_SPEC)
}
fn letta_match(t: &str) -> Option<ClassificationResult> {
    run_spec(t, &LETTA_SPEC)
}
fn pi_match(t: &str) -> Option<ClassificationResult> {
    run_spec(t, &PI_SPEC)
}
fn qwen_match(t: &str) -> Option<ClassificationResult> {
    run_spec(t, &QWEN_SPEC)
}

pub fn auggie_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("auggie", auggie_match)
}
pub fn charm_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("charm", charm_match)
}
pub fn codebuff_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("codebuff", codebuff_match)
}
pub fn freebuff_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("freebuff", freebuff_match)
}
pub fn generic_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("generic", generic_match)
}
pub fn goose_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("goose", goose_match)
}
pub fn kimi_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("kimi", kimi_match)
}
pub fn letta_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("letta", letta_match)
}
pub fn mistral_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("mistral", mistral_match)
}
pub fn pi_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("pi", pi_match)
}
pub fn qwen_classifier() -> BufferedTextClassifier {
    BufferedTextClassifier::new("qwen", qwen_match)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_hooks::text_classifier::TextClassifier;

    fn assert_kind(r: Option<ClassificationResult>, expected: AgentEventKind) {
        let r = r.expect("expected a classification");
        assert_eq!(r.kind, expected);
    }

    #[test]
    fn auggie_charm_codebuff_freebuff_goose_kimi_qwen_match_default_idle() {
        for c in [
            auggie_classifier(),
            charm_classifier(),
            codebuff_classifier(),
            freebuff_classifier(),
            goose_classifier(),
            kimi_classifier(),
            qwen_classifier(),
        ] {
            let name = c.name();
            assert_kind(
                c.feed("\nReady for input"),
                AgentEventKind::Notification {
                    notification_kind: NotificationKind::IdlePrompt,
                },
            );
            // sanity: standard PERM_DEFAULT pattern still fires
            assert_kind(
                c.feed("approve this?"),
                AgentEventKind::Notification {
                    notification_kind: NotificationKind::PermissionPrompt,
                },
            );
            let _ = name;
        }
    }

    #[test]
    fn charm_recognizes_crush_prompt_as_idle() {
        let c = charm_classifier();
        assert_kind(
            c.feed("\ncrush> "),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::IdlePrompt,
            },
        );
    }

    #[test]
    fn generic_emits_stop_on_checkmark() {
        let c = generic_classifier();
        let r = c.feed("✓ all done").unwrap();
        assert_eq!(r.kind, AgentEventKind::Stop);
    }

    #[test]
    fn generic_perm_on_bracket_yn() {
        let c = generic_classifier();
        assert_kind(
            c.feed("[y/n]"),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::PermissionPrompt,
            },
        );
    }

    #[test]
    fn goose_recognizes_session_started_as_idle() {
        let c = goose_classifier();
        assert_kind(
            c.feed("Session foo started"),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::IdlePrompt,
            },
        );
    }

    #[test]
    fn letta_idle_on_slash_command_hint() {
        let c = letta_classifier();
        assert_kind(
            c.feed("/help available"),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::IdlePrompt,
            },
        );
    }

    #[test]
    fn letta_auth_includes_successfully_connected() {
        let c = letta_classifier();
        assert_kind(
            c.feed("Successfully connected to host"),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::AuthSuccess,
            },
        );
    }

    #[test]
    fn pi_emits_stop_on_agent_end_json() {
        let c = pi_classifier();
        let r = c.feed(r#"{"type": "agent_end"}"#).unwrap();
        assert_eq!(r.kind, AgentEventKind::Stop);
    }

    #[test]
    fn qwen_emits_stop_on_task_completed() {
        let c = qwen_classifier();
        let r = c.feed("Task completed").unwrap();
        assert_eq!(r.kind, AgentEventKind::Stop);
    }

    #[test]
    fn mistral_short_tail_idle_marker_fires() {
        let c = mistral_classifier();
        assert_kind(
            c.feed("vibe> "),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::IdlePrompt,
            },
        );
    }

    #[test]
    fn mistral_long_tail_does_not_fire_on_bare_arrow() {
        let c = mistral_classifier();
        // Tail >= 100 chars: the `>` length-gated idle must not fire.
        let long = "x".repeat(200) + " > end";
        // mistral has no idle for that text and no auth/elicit/error
        // pattern matches; should fall through to None.
        assert!(c.feed(&long).is_none(), "mistral fired on long >");
    }

    #[test]
    fn mistral_uses_bespoke_permission_pattern() {
        let c = mistral_classifier();
        assert_kind(
            c.feed("Continue?"),
            AgentEventKind::Notification {
                notification_kind: NotificationKind::PermissionPrompt,
            },
        );
    }
}
