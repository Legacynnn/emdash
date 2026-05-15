//! Family-specific text classifiers (EMD-23/24/25 ports).
//!
//! Each agent has a small module here that implements the
//! `TextClassifier` trait. Every classifier defers to
//! `classify_common` for the shared pattern set and just supplies
//! its own `idle_prompt` regex (the one regex that differs across
//! agent-style classifiers in the Electron build).

pub mod family_a;
pub mod family_b;
pub mod family_c;

pub use family_a::{
    amp_classifier, autohand_classifier, cursor_classifier, devin_classifier, jules_classifier,
    opencode_classifier, rovo_classifier,
};
pub use family_b::{
    cline_classifier, continue_classifier, copilot_classifier, droid_classifier, gemini_classifier,
    junie_classifier, kilocode_classifier, kiro_classifier,
};
pub use family_c::{
    auggie_classifier, charm_classifier, codebuff_classifier, freebuff_classifier,
    generic_classifier, goose_classifier, kimi_classifier, letta_classifier, mistral_classifier,
    pi_classifier, qwen_classifier,
};
