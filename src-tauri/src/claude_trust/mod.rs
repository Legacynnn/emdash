//! Claude Code worktree auto-trust (EMD-26).
//!
//! When Emdash spawns claude inside a fresh worktree, Claude Code's
//! first-launch trust dialog blocks until the user accepts. The
//! Electron build's `ClaudeTrustService` (`core/agent-hooks/
//! claude-trust-service.ts`) sidesteps that by writing
//! `~/.claude.json` with `projects.<worktree>.hasTrustDialogAccepted
//! = true` and `hasCompletedProjectOnboarding = true` before the
//! agent launches.
//!
//! This port mirrors the Electron behavior for the local case only.
//! Remote/SSH auto-trust is deferred to EMD-10 (which lands the
//! whole SSH remote-FS abstraction).
//!
//! Safety:
//!
//! - Per-config-path mutex serializes concurrent writes (two agents
//!   spawning at once would otherwise race on the JSON file).
//! - Writes are atomic: temp file + `rename`. The temp suffix is a
//!   UUID so two processes can't collide either.
//! - Refuses to overwrite a non-object config root (preserves user
//!   data) and refuses to overwrite a corrupt JSON config (preserves
//!   the user's chance to recover by hand).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::{Map, Value};
use thiserror::Error;
use uuid::Uuid;

pub const CLAUDE_CONFIG_NAME: &str = ".claude.json";

#[derive(Debug, Error)]
pub enum ClaudeTrustError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde_json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Default)]
pub struct ClaudeTrustService {
    /// Per-config-path serialization. Built on the same shape as
    /// `tasks::fs_lock::WorkspaceFsMutationLock`, but a separate
    /// instance because the keys are config paths, not workspace ids.
    locks: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
}

impl ClaudeTrustService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Auto-trust `worktree_path` in `<home_dir>/.claude.json`.
    /// Idempotent: no-op (and no write) if the worktree is already
    /// trusted. Errors from I/O or JSON parsing surface to the
    /// caller; the Electron port logs-and-swallows in the same
    /// situation but we propagate so EMD-27's spawn site can decide.
    pub fn auto_trust_local(
        &self,
        worktree_path: &Path,
        home_dir: &Path,
    ) -> Result<(), ClaudeTrustError> {
        let config_path = home_dir.join(CLAUDE_CONFIG_NAME);
        let lock = self.lock_for(&config_path);
        let _guard = lock.lock();

        // canonicalize the worktree path so the key in `projects.<path>`
        // matches what Claude Code will look up. canonicalize fails if
        // the path doesn't exist yet — fall back to the literal path.
        let normalized = worktree_path
            .canonicalize()
            .unwrap_or_else(|_| worktree_path.to_path_buf());
        let key = normalized.to_string_lossy().into_owned();

        let raw = read_config(&config_path)?;
        let config = match parse_config(raw.as_deref()) {
            ParsedConfig::Object(obj) => obj,
            ParsedConfig::Skip => return Ok(()),
        };
        let Some(next) = with_trusted_project(config, &key) else {
            // Already trusted — nothing to write.
            return Ok(());
        };
        let serialized = serde_json::to_string_pretty(&Value::Object(next))? + "\n";
        atomic_write(&config_path, &serialized)?;
        Ok(())
    }

    fn lock_for(&self, path: &Path) -> Arc<Mutex<()>> {
        let mut guard = self.locks.lock();
        guard
            .entry(path.to_path_buf())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}

enum ParsedConfig {
    Object(Map<String, Value>),
    /// Non-object root or unparseable JSON. Skip rather than
    /// overwrite — both cases would clobber user data the user can
    /// still recover by hand.
    Skip,
}

fn parse_config(raw: Option<&str>) -> ParsedConfig {
    match raw {
        None => ParsedConfig::Object(Map::new()),
        Some(s) if s.trim().is_empty() => ParsedConfig::Object(Map::new()),
        Some(s) => match serde_json::from_str::<Value>(s) {
            Ok(Value::Object(obj)) => ParsedConfig::Object(obj),
            _ => ParsedConfig::Skip,
        },
    }
}

fn with_trusted_project(
    mut config: Map<String, Value>,
    worktree_path: &str,
) -> Option<Map<String, Value>> {
    let mut projects = match config.remove("projects") {
        Some(Value::Object(o)) => o,
        _ => Map::new(),
    };
    let mut existing = match projects.remove(worktree_path) {
        Some(Value::Object(o)) => o,
        _ => Map::new(),
    };

    let already_trusted = existing.get("hasTrustDialogAccepted") == Some(&Value::Bool(true))
        && existing.get("hasCompletedProjectOnboarding") == Some(&Value::Bool(true));
    if already_trusted {
        // Put the unchanged map back and signal "no-op".
        projects.insert(worktree_path.to_string(), Value::Object(existing));
        config.insert("projects".to_string(), Value::Object(projects));
        return None;
    }

    existing.insert("hasTrustDialogAccepted".to_string(), Value::Bool(true));
    existing.insert(
        "hasCompletedProjectOnboarding".to_string(),
        Value::Bool(true),
    );
    projects.insert(worktree_path.to_string(), Value::Object(existing));
    config.insert("projects".to_string(), Value::Object(projects));
    Some(config)
}

fn read_config(path: &Path) -> Result<Option<String>, ClaudeTrustError> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn atomic_write(path: &Path, content: &str) -> Result<(), ClaudeTrustError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(
        "{}.{}.tmp",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("config"),
        Uuid::new_v4()
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

    fn read_json(path: &Path) -> Value {
        let s = std::fs::read_to_string(path).unwrap();
        serde_json::from_str(&s).unwrap()
    }

    #[test]
    fn creates_config_if_missing() {
        let home = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        let svc = ClaudeTrustService::new();
        svc.auto_trust_local(wt.path(), home.path()).unwrap();
        let json = read_json(&home.path().join(CLAUDE_CONFIG_NAME));
        let key = wt
            .path()
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert_eq!(
            json["projects"][&key]["hasTrustDialogAccepted"],
            Value::Bool(true)
        );
        assert_eq!(
            json["projects"][&key]["hasCompletedProjectOnboarding"],
            Value::Bool(true)
        );
    }

    #[test]
    fn preserves_unrelated_keys() {
        let home = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        let path = home.path().join(CLAUDE_CONFIG_NAME);
        std::fs::write(
            &path,
            r#"{"userId":"u-1","other":{"keep":true},"projects":{"/old":{"hasTrustDialogAccepted":true,"hasCompletedProjectOnboarding":true}}}"#,
        )
        .unwrap();
        let svc = ClaudeTrustService::new();
        svc.auto_trust_local(wt.path(), home.path()).unwrap();
        let json = read_json(&path);
        assert_eq!(json["userId"], Value::String("u-1".into()));
        assert_eq!(json["other"]["keep"], Value::Bool(true));
        // Old project entry still present.
        assert_eq!(
            json["projects"]["/old"]["hasTrustDialogAccepted"],
            Value::Bool(true)
        );
    }

    #[test]
    fn idempotent_no_rewrite_when_already_trusted() {
        let home = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        let svc = ClaudeTrustService::new();
        svc.auto_trust_local(wt.path(), home.path()).unwrap();
        let path = home.path().join(CLAUDE_CONFIG_NAME);
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        // Sleep enough for mtime granularity (HFS+/APFS = 1µs, but
        // be generous so the test doesn't flake on slow CI).
        std::thread::sleep(std::time::Duration::from_millis(10));
        svc.auto_trust_local(wt.path(), home.path()).unwrap();
        let after = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(before, after, "second call should be a no-op");
    }

    #[test]
    fn skips_non_object_root() {
        let home = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        let path = home.path().join(CLAUDE_CONFIG_NAME);
        std::fs::write(&path, "[1, 2, 3]").unwrap();
        let svc = ClaudeTrustService::new();
        svc.auto_trust_local(wt.path(), home.path()).unwrap();
        // Untouched.
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[1, 2, 3]");
    }

    #[test]
    fn skips_corrupt_json() {
        let home = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        let path = home.path().join(CLAUDE_CONFIG_NAME);
        std::fs::write(&path, "{this is not json").unwrap();
        let svc = ClaudeTrustService::new();
        svc.auto_trust_local(wt.path(), home.path()).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{this is not json");
    }

    #[test]
    fn empty_file_starts_fresh() {
        let home = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        let path = home.path().join(CLAUDE_CONFIG_NAME);
        std::fs::write(&path, "").unwrap();
        let svc = ClaudeTrustService::new();
        svc.auto_trust_local(wt.path(), home.path()).unwrap();
        let json = read_json(&path);
        assert!(json["projects"].is_object());
    }
}
