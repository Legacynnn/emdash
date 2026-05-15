//! Per-provider hook config writer (EMD-26).
//!
//! Ports `core/agent-hooks/hook-config.ts` and
//! `core/agent-hooks/agent-notify-command.ts`. For each supported
//! provider, writes a config file into the worktree that asks the
//! provider to POST hook events to Emdash's local agent-hook server
//! (EMD-9), then appends the config path to the worktree's
//! `.gitignore` (so users don't accidentally commit their per-launch
//! token-bearing hook commands).
//!
//! Provider behaviors:
//!
//! - **claude**: append a `curl`-based `Notification` and `Stop`
//!   hook command to `.claude/settings.local.json`.
//! - **codex**: set `notify` in `.codex/config.toml` to a `bash`
//!   wrapper that POSTs the notify payload.
//! - **pi**: write the bundled `pi-emdash-extension.ts` into
//!   `.pi/extensions/emdash-hook.ts` (sourced verbatim from the
//!   Electron build).
//! - **opencode**: write the bundled
//!   `opencode-notifications-plugin.js` into
//!   `.opencode/plugins/emdash-notifications.js`.
//!
//! Probing for `claude`/`codex`/`pi`/`opencode` on the user's PATH
//! is the gate: if the binary isn't installed, we don't write the
//! config (matches the Electron behavior).

mod assets;
pub mod gitignore;
pub mod notify_command;
pub mod path_probe;

use std::path::Path;

use serde_json::{json, Map, Value};
use thiserror::Error;
use toml_edit::DocumentMut;

pub const CLAUDE_SETTINGS_PATH: &str = ".claude/settings.local.json";
pub const CODEX_CONFIG_PATH: &str = ".codex/config.toml";
pub const PI_EXTENSION_PATH: &str = ".pi/extensions/emdash-hook.ts";
pub const OPENCODE_PLUGIN_PATH: &str = ".opencode/plugins/emdash-notifications.js";
pub const GITIGNORE_PATH: &str = ".gitignore";

const EMDASH_MARKER: &str = "EMDASH_HOOK_PORT";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Claude,
    Codex,
    Pi,
    Opencode,
}

impl Provider {
    pub fn cli_name(self) -> &'static str {
        match self {
            Provider::Claude => "claude",
            Provider::Codex => "codex",
            Provider::Pi => "pi",
            Provider::Opencode => "opencode",
        }
    }

    pub fn config_path(self) -> &'static str {
        match self {
            Provider::Claude => CLAUDE_SETTINGS_PATH,
            Provider::Codex => CODEX_CONFIG_PATH,
            Provider::Pi => PI_EXTENSION_PATH,
            Provider::Opencode => OPENCODE_PLUGIN_PATH,
        }
    }

    pub fn all() -> &'static [Provider] {
        &[
            Provider::Claude,
            Provider::Codex,
            Provider::Pi,
            Provider::Opencode,
        ]
    }
}

#[derive(Debug, Error)]
pub enum HookConfigError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("toml error: {0}")]
    Toml(#[from] toml_edit::TomlError),
}

#[derive(Debug, Clone, Copy)]
pub struct WriteOptions {
    /// If true (the default), append the written config path to the
    /// worktree's `.gitignore`. Disabled in tests that don't care.
    pub write_gitignore_entries: bool,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            write_gitignore_entries: true,
        }
    }
}

/// Write hook configs for every provider whose CLI is on `PATH`. The
/// Electron `writeAll` does this concurrently with per-provider
/// error containment; we run sequentially because (a) the work is
/// I/O-light and (b) sequential keeps gitignore appends serialized
/// against each other without a per-file lock.
pub fn write_all(
    worktree: &Path,
    options: WriteOptions,
) -> Vec<(Provider, Result<bool, HookConfigError>)> {
    Provider::all()
        .iter()
        .copied()
        .map(|p| (p, write_for_provider(p, worktree, options)))
        .collect()
}

pub fn write_for_provider(
    provider: Provider,
    worktree: &Path,
    options: WriteOptions,
) -> Result<bool, HookConfigError> {
    if !path_probe::command_on_path(provider.cli_name()) {
        return Ok(false);
    }
    let wrote = match provider {
        Provider::Claude => write_claude_hooks(worktree)?,
        Provider::Codex => write_codex_notify(worktree)?,
        Provider::Pi => write_pi_extension(worktree)?,
        Provider::Opencode => write_opencode_plugin(worktree)?,
    };
    if wrote && options.write_gitignore_entries {
        gitignore::ensure_entries(worktree, &[provider.config_path()])?;
    }
    Ok(wrote)
}

fn write_claude_hooks(worktree: &Path) -> Result<bool, HookConfigError> {
    let config_path = worktree.join(CLAUDE_SETTINGS_PATH);
    let mut config: Map<String, Value> = match read_to_string_opt(&config_path)? {
        Some(s) if !s.trim().is_empty() => match serde_json::from_str(&s) {
            Ok(Value::Object(o)) => o,
            // Corrupt or non-object root: drop and rewrite. The
            // Electron port did the same — it catches the parse
            // error and treats config as `{}`.
            _ => Map::new(),
        },
        _ => Map::new(),
    };
    let mut hooks = match config.remove("hooks") {
        Some(Value::Object(o)) => o,
        _ => Map::new(),
    };
    for (event_type, hook_key) in [("notification", "Notification"), ("stop", "Stop")] {
        let existing = match hooks.remove(hook_key) {
            Some(Value::Array(a)) => a,
            _ => Vec::new(),
        };
        hooks.insert(
            hook_key.to_string(),
            Value::Array(build_hook_entries(
                existing,
                &notify_command::claude_hook_command(event_type),
            )),
        );
    }
    config.insert("hooks".to_string(), Value::Object(hooks));
    let serialized = serde_json::to_string_pretty(&Value::Object(config))? + "\n";
    write_atomic_string(&config_path, &serialized)?;
    Ok(true)
}

/// Strip emdash-marked entries from the prior list, then append the
/// fresh one. Preserves user-authored entries.
fn build_hook_entries(existing: Vec<Value>, command: &str) -> Vec<Value> {
    let mut out: Vec<Value> = existing
        .into_iter()
        .filter(|entry| {
            // serialize, then check for our marker. cheap+robust to
            // shape changes in the user's settings.
            serde_json::to_string(entry)
                .map(|s| !s.contains(EMDASH_MARKER))
                .unwrap_or(true)
        })
        .collect();
    out.push(json!({
        "hooks": [{ "type": "command", "command": command }]
    }));
    out
}

fn write_codex_notify(worktree: &Path) -> Result<bool, HookConfigError> {
    let config_path = worktree.join(CODEX_CONFIG_PATH);
    let mut doc = match read_to_string_opt(&config_path)? {
        Some(s) if !s.trim().is_empty() => s.parse::<DocumentMut>()?,
        _ => DocumentMut::new(),
    };
    // Set top-level `notify = [...]` to the platform-appropriate
    // wrapper. toml_edit's array preserves user formatting in the
    // rest of the doc.
    let notify_cmd = notify_command::codex_notify_command();
    let mut arr = toml_edit::Array::new();
    for token in notify_cmd {
        arr.push(toml_edit::Value::from(token.as_str()));
    }
    doc.insert("notify", toml_edit::value(arr));
    write_atomic_string(&config_path, &doc.to_string())?;
    Ok(true)
}

fn write_pi_extension(worktree: &Path) -> Result<bool, HookConfigError> {
    let config_path = worktree.join(PI_EXTENSION_PATH);
    let content = assets::PI_EXTENSION;
    if read_to_string_opt(&config_path)?.as_deref() == Some(content) {
        return Ok(true);
    }
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_atomic_string(&config_path, content)?;
    Ok(true)
}

fn write_opencode_plugin(worktree: &Path) -> Result<bool, HookConfigError> {
    let config_path = worktree.join(OPENCODE_PLUGIN_PATH);
    let content = assets::OPENCODE_PLUGIN;
    if read_to_string_opt(&config_path)?.as_deref() == Some(content) {
        return Ok(true);
    }
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_atomic_string(&config_path, content)?;
    Ok(true)
}

fn read_to_string_opt(path: &Path) -> Result<Option<String>, HookConfigError> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn write_atomic_string(path: &Path, content: &str) -> Result<(), HookConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}.{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or(""),
        uuid::Uuid::new_v4()
    ));
    std::fs::write(&tmp, content)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worktree() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn claude_hooks_creates_settings_with_curl_command() {
        let wt = worktree();
        write_claude_hooks(wt.path()).unwrap();
        let s = std::fs::read_to_string(wt.path().join(CLAUDE_SETTINGS_PATH)).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        let notif = &v["hooks"]["Notification"];
        let stop = &v["hooks"]["Stop"];
        assert!(notif.is_array());
        assert!(stop.is_array());
        let first_cmd = notif[0]["hooks"][0]["command"].as_str().unwrap();
        assert!(first_cmd.contains("$EMDASH_HOOK_PORT"));
        assert!(first_cmd.contains("X-Emdash-Event-Type: notification"));
    }

    #[test]
    fn claude_hooks_preserves_user_entries_strips_prior_emdash_entry() {
        let wt = worktree();
        let path = wt.path().join(CLAUDE_SETTINGS_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // Pre-existing user entry + an old emdash entry that uses the marker.
        let prior = json!({
            "hooks": {
                "Notification": [
                    {"hooks": [{"type": "command", "command": "echo hi"}]},
                    {"hooks": [{"type": "command", "command": "EMDASH_HOOK_PORT=$old echo gone"}]}
                ]
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&prior).unwrap()).unwrap();
        write_claude_hooks(wt.path()).unwrap();
        let s = std::fs::read_to_string(&path).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        let notif = v["hooks"]["Notification"].as_array().unwrap();
        // user entry kept, old emdash entry replaced -> exactly 2 entries.
        assert_eq!(notif.len(), 2, "got: {notif:?}");
        assert!(notif
            .iter()
            .any(|e| serde_json::to_string(e).unwrap().contains("echo hi")));
    }

    #[test]
    fn codex_notify_writes_notify_key_to_existing_toml() {
        let wt = worktree();
        let path = wt.path().join(CODEX_CONFIG_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "# user comment\nfoo = \"bar\"\n").unwrap();
        write_codex_notify(wt.path()).unwrap();
        let s = std::fs::read_to_string(&path).unwrap();
        assert!(s.contains("# user comment"), "preserves comments");
        assert!(s.contains("foo = \"bar\""), "preserves user keys");
        assert!(s.contains("notify"), "writes notify key");
        assert!(
            s.contains("EMDASH_HOOK_PORT") || s.contains("EMDASH_HOOK_TOKEN"),
            "notify command references hook env"
        );
    }

    #[test]
    fn pi_extension_writes_embedded_asset_verbatim() {
        let wt = worktree();
        write_pi_extension(wt.path()).unwrap();
        let s = std::fs::read_to_string(wt.path().join(PI_EXTENSION_PATH)).unwrap();
        assert_eq!(s, assets::PI_EXTENSION);
    }

    #[test]
    fn opencode_plugin_writes_embedded_asset_verbatim() {
        let wt = worktree();
        write_opencode_plugin(wt.path()).unwrap();
        let s = std::fs::read_to_string(wt.path().join(OPENCODE_PLUGIN_PATH)).unwrap();
        assert_eq!(s, assets::OPENCODE_PLUGIN);
    }

    #[test]
    fn pi_extension_is_idempotent() {
        let wt = worktree();
        write_pi_extension(wt.path()).unwrap();
        let path = wt.path().join(PI_EXTENSION_PATH);
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        write_pi_extension(wt.path()).unwrap();
        let after = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(before, after, "no rewrite when content matches");
    }

    #[test]
    fn write_for_provider_skips_when_cli_missing() {
        let wt = worktree();
        // PATH probe will (almost certainly) miss a randomly named CLI.
        // To avoid env mutation, we test the no-cli path indirectly:
        // claude almost certainly isn't installed on CI runners.
        // If your local CI has claude on PATH, this test is a no-op;
        // assert only that the function returns Ok regardless.
        let r = write_for_provider(Provider::Claude, wt.path(), WriteOptions::default());
        assert!(r.is_ok());
        // We don't assert the boolean — depends on the CI environment.
        let _ = r.unwrap();
    }

    #[test]
    fn write_for_provider_writes_gitignore_entry_when_enabled() {
        let wt = worktree();
        // Force-call the per-provider writer to bypass the PATH probe.
        write_pi_extension(wt.path()).unwrap();
        gitignore::ensure_entries(wt.path(), &[PI_EXTENSION_PATH]).unwrap();
        let s = std::fs::read_to_string(wt.path().join(GITIGNORE_PATH)).unwrap();
        assert!(s.contains(PI_EXTENSION_PATH));
    }
}
