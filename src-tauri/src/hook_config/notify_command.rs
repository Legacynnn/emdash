//! Renders the per-provider hook command strings that
//! `hook_config::write_*` injects into the generated config files.
//! Ports `core/agent-hooks/agent-notify-command.ts`.
//!
//! Two important notes:
//!
//! - Env-var names are the Electron-compatible ones
//!   (`EMDASH_HOOK_PORT`, `EMDASH_HOOK_TOKEN`, `EMDASH_PTY_ID`)
//!   matching `agent_hooks::env::inject_hook_env_into`.
//! - The Windows codex notify command needs a PowerShell helper
//!   script written to disk; we render the script body here and
//!   leave script materialization to the caller.

/// `claude_hook_command(eventType)` → the curl command Claude's
/// hook system will run for `Notification` / `Stop` events. The TS
/// equivalent is `makeClaudeHookCommand` in
/// `agent-notify-command.ts`.
pub fn claude_hook_command(event_type: &str) -> String {
    let mut s = String::from("curl -sf -X POST ");
    s.push_str("-H \"Content-Type: application/json\" ");
    s.push_str("-H \"X-Emdash-Token: $EMDASH_HOOK_TOKEN\" ");
    s.push_str("-H \"X-Emdash-Pty-Id: $EMDASH_PTY_ID\" ");
    s.push_str(&format!("-H \"X-Emdash-Event-Type: {event_type}\" "));
    s.push_str("-d @- ");
    s.push_str("\"http://127.0.0.1:$EMDASH_HOOK_PORT/hook\" || true");
    s
}

/// Codex's `notify` config is a list of argv tokens. On POSIX we
/// shell out to `bash -c`; on Windows we shell out to PowerShell
/// (which requires the script-file dance, kept out of scope here so
/// this function is pure).
pub fn codex_notify_command() -> Vec<String> {
    if cfg!(windows) {
        windows_codex_notify_argv()
    } else {
        posix_codex_notify_argv()
    }
}

/// The PowerShell script the Windows codex `notify` wrapper points
/// at. The caller is expected to write this to disk once at startup
/// and pass the resulting path to `windows_codex_notify_argv`.
pub fn windows_codex_notify_script_body() -> String {
    [
        "param([string]$payload)",
        "try {",
        "  Invoke-WebRequest -UseBasicParsing -Method POST \
-Uri ('http://127.0.0.1:' + $env:EMDASH_HOOK_PORT + '/hook') \
-Headers @{ 'Content-Type' = 'application/json'; \
'X-Emdash-Token' = $env:EMDASH_HOOK_TOKEN; \
'X-Emdash-Pty-Id' = $env:EMDASH_PTY_ID; \
'X-Emdash-Event-Type' = 'notification' \
} -Body $payload | Out-Null",
        "} catch {",
        "  exit 0",
        "}",
        "",
    ]
    .join("\n")
}

fn posix_codex_notify_argv() -> Vec<String> {
    vec![
        "bash".to_string(),
        "-c".to_string(),
        // Same body as the TS source; quoting matches.
        "curl -sf -X POST \
-H 'Content-Type: application/json' \
-H \"X-Emdash-Token: $EMDASH_HOOK_TOKEN\" \
-H \"X-Emdash-Pty-Id: $EMDASH_PTY_ID\" \
-H \"X-Emdash-Event-Type: notification\" \
-d \"$1\" \
\"http://127.0.0.1:$EMDASH_HOOK_PORT/hook\" || true"
            .to_string(),
        "_".to_string(),
    ]
}

fn windows_codex_notify_argv() -> Vec<String> {
    // The caller substitutes the actual script path before passing
    // this to codex. We render a placeholder here — the Electron
    // build does the same and overrides the path at call time.
    vec![
        "powershell.exe".to_string(),
        "-NoProfile".to_string(),
        "-File".to_string(),
        // Placeholder — see ADR-0028 for the script-on-disk dance.
        // Tests substitute this after the fact.
        "<emdash-codex-notify.ps1>".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_hook_command_contains_event_type_and_hook_env() {
        let cmd = claude_hook_command("notification");
        assert!(cmd.contains("X-Emdash-Event-Type: notification"));
        assert!(cmd.contains("$EMDASH_HOOK_TOKEN"));
        assert!(cmd.contains("$EMDASH_HOOK_PORT"));
        assert!(cmd.contains("$EMDASH_PTY_ID"));
    }

    #[test]
    fn posix_codex_notify_argv_starts_with_bash_dash_c() {
        let argv = posix_codex_notify_argv();
        assert_eq!(argv[0], "bash");
        assert_eq!(argv[1], "-c");
        assert!(argv[2].contains("EMDASH_HOOK_PORT"));
        assert_eq!(argv[3], "_");
    }

    #[test]
    fn windows_codex_script_body_invokes_webrequest() {
        let body = windows_codex_notify_script_body();
        assert!(body.contains("Invoke-WebRequest"));
        assert!(body.contains("EMDASH_HOOK_PORT"));
        assert!(body.contains("X-Emdash-Token"));
    }
}
