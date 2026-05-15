//! Helper for the agent-spawn site (EMD-27) to inject hook
//! coordinates into the child process's env. Documented as a
//! stub here; EMD-27 wires it into the PTY spawn path.

use std::collections::HashMap;

/// Inject the running hook server's `port` + `token` into `env` so a
/// child agent process knows how to reach back. Variable names match
/// what the Electron build set (no change to Claude Code's
/// configured `CLAUDE_HOOKS_URL` semantics).
pub fn inject_hook_env_into(env: &mut HashMap<String, String>, port: u16, token: &str) {
    env.insert("EMDASH_AGENT_HOOK_PORT".to_string(), port.to_string());
    env.insert("EMDASH_AGENT_HOOK_TOKEN".to_string(), token.to_string());
    // Convenience aggregate — some agents accept a single URL.
    env.insert(
        "EMDASH_AGENT_HOOK_URL".to_string(),
        format!("http://127.0.0.1:{port}/hook"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_all_three_vars() {
        let mut env = HashMap::new();
        inject_hook_env_into(&mut env, 12345, "abc-token");
        assert_eq!(env.get("EMDASH_AGENT_HOOK_PORT").unwrap(), "12345");
        assert_eq!(env.get("EMDASH_AGENT_HOOK_TOKEN").unwrap(), "abc-token");
        assert_eq!(
            env.get("EMDASH_AGENT_HOOK_URL").unwrap(),
            "http://127.0.0.1:12345/hook"
        );
    }

    #[test]
    fn does_not_clobber_unrelated_keys() {
        let mut env = HashMap::new();
        env.insert("PATH".into(), "/usr/bin".into());
        inject_hook_env_into(&mut env, 1, "t");
        assert_eq!(env.get("PATH").unwrap(), "/usr/bin");
    }
}
