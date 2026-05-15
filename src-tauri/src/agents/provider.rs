//! Provider config — the agents emdash-dev knows how to spawn.
//!
//! v1 stub: hard-coded binary name + default args per provider. A
//! later issue can move this into a user-extensible config layer
//! (settings table or a manifest file under `~/.emdash/agents/`).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use specta::Type;

/// Stable identifier the renderer passes back when starting an
/// agent. New variants land in this enum (and the `provider_spec`
/// match) per agent supported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AgentProvider {
    Codex,
    Claude,
    Cursor,
    Opencode,
    Continue,
    Cline,
}

/// Static spec for a provider — the literal binary name on PATH +
/// default CLI args. The renderer can render the spec to a
/// provider picker without round-tripping per spawn.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
pub struct ProviderSpec {
    pub provider: AgentProvider,
    /// Display name shown in the renderer's picker.
    pub label: &'static str,
    /// Binary to spawn (looked up on PATH).
    pub binary: &'static str,
    /// Default args appended after the binary.
    pub args: Vec<String>,
    /// Per-agent env overrides merged on top of the login-shell env
    /// before hook-env injection.
    pub env: HashMap<String, String>,
}

pub fn provider_spec(provider: AgentProvider) -> ProviderSpec {
    let label_binary_args: (&'static str, &'static str, Vec<String>) = match provider {
        AgentProvider::Codex => ("Codex", "codex", vec![]),
        AgentProvider::Claude => ("Claude Code", "claude", vec![]),
        AgentProvider::Cursor => ("Cursor Agent", "cursor-agent", vec![]),
        AgentProvider::Opencode => ("OpenCode", "opencode", vec![]),
        AgentProvider::Continue => ("Continue", "continue", vec![]),
        AgentProvider::Cline => ("Cline", "cline", vec![]),
    };
    let (label, binary, args) = label_binary_args;
    let env = match provider {
        // Claude Code reads CLAUDE_CODE_HOOKS_URL when present; we
        // forward our `inject_hook_env_into` URL through the
        // standard EMDASH_AGENT_HOOK_URL too so the provider can
        // pick whichever it prefers.
        AgentProvider::Claude => HashMap::new(),
        _ => HashMap::new(),
    };
    ProviderSpec {
        provider,
        label,
        binary,
        args,
        env,
    }
}

/// Iterate every known provider in display order. Useful for the
/// renderer's "available providers" list.
pub fn known_providers() -> Vec<ProviderSpec> {
    [
        AgentProvider::Codex,
        AgentProvider::Claude,
        AgentProvider::Cursor,
        AgentProvider::Opencode,
        AgentProvider::Continue,
        AgentProvider::Cline,
    ]
    .into_iter()
    .map(provider_spec)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_provider_has_nonempty_binary() {
        for spec in known_providers() {
            assert!(!spec.binary.is_empty(), "{:?}", spec.provider);
            assert!(!spec.label.is_empty(), "{:?}", spec.provider);
        }
    }
}
