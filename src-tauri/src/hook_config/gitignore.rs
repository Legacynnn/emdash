//! `.gitignore` append helper. Ports the gitignore-management piece
//! of `hook-config.ts` so the hook config files we write don't get
//! accidentally committed.

use std::path::Path;

use super::{HookConfigError, GITIGNORE_PATH};

/// Append `entries` to the worktree's `.gitignore` if they're not
/// already covered. "Already covered" is a simple match:
///
/// - exact line equality (after leading-`/` strip)
/// - `dir/` prefix matches a path prefixed with `dir/`
/// - `dir/**` matches a path prefixed with `dir/`
///
/// Anything fancier (negation, glob stars in the middle) is treated
/// as not covering, so we'll append a more-specific rule. The
/// Electron source uses the same simplification.
pub fn ensure_entries(worktree: &Path, entries: &[&str]) -> Result<(), HookConfigError> {
    let path = worktree.join(GITIGNORE_PATH);
    let existing = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    let existing_lines: Vec<String> = existing
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let missing: Vec<&&str> = entries
        .iter()
        .filter(|e| !is_gitignored(&existing_lines, e))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let trimmed = existing.trim_end().to_string();
    let appended = missing
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let next = if trimmed.is_empty() {
        format!("{appended}\n")
    } else {
        format!("{trimmed}\n{appended}\n")
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, next)?;
    Ok(())
}

fn is_gitignored(existing_entries: &[String], entry: &str) -> bool {
    let normalized = entry.trim_start_matches('/');
    existing_entries.iter().any(|raw| {
        let pat = raw.trim_start_matches('/');
        if pat == normalized {
            return true;
        }
        if let Some(prefix) = pat.strip_suffix("/**") {
            return normalized.starts_with(prefix);
        }
        if pat.ends_with('/') {
            return normalized.starts_with(pat);
        }
        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_gitignore_when_missing() {
        let wt = tempfile::tempdir().unwrap();
        ensure_entries(wt.path(), &[".claude/settings.local.json"]).unwrap();
        let s = std::fs::read_to_string(wt.path().join(".gitignore")).unwrap();
        assert!(s.contains(".claude/settings.local.json"));
    }

    #[test]
    fn appends_only_missing_entries() {
        let wt = tempfile::tempdir().unwrap();
        std::fs::write(
            wt.path().join(".gitignore"),
            "node_modules\n.claude/settings.local.json\n",
        )
        .unwrap();
        ensure_entries(
            wt.path(),
            &[".claude/settings.local.json", ".codex/config.toml"],
        )
        .unwrap();
        let s = std::fs::read_to_string(wt.path().join(".gitignore")).unwrap();
        // Existing entries unchanged.
        assert!(s.contains("node_modules"));
        // Both old + new claude line, but only one (the original).
        assert_eq!(s.matches(".claude/settings.local.json").count(), 1);
        assert!(s.contains(".codex/config.toml"));
    }

    #[test]
    fn directory_prefix_pattern_covers_child_path() {
        let wt = tempfile::tempdir().unwrap();
        std::fs::write(wt.path().join(".gitignore"), ".claude/\n").unwrap();
        ensure_entries(wt.path(), &[".claude/settings.local.json"]).unwrap();
        let s = std::fs::read_to_string(wt.path().join(".gitignore")).unwrap();
        // `.claude/` already covers it.
        assert_eq!(s.matches(".claude").count(), 1);
    }

    #[test]
    fn double_star_pattern_covers_child_path() {
        let wt = tempfile::tempdir().unwrap();
        std::fs::write(wt.path().join(".gitignore"), ".pi/**\n").unwrap();
        ensure_entries(wt.path(), &[".pi/extensions/emdash-hook.ts"]).unwrap();
        let s = std::fs::read_to_string(wt.path().join(".gitignore")).unwrap();
        assert_eq!(s.matches(".pi").count(), 1);
    }

    #[test]
    fn ignores_comments_when_checking_coverage() {
        let wt = tempfile::tempdir().unwrap();
        std::fs::write(
            wt.path().join(".gitignore"),
            "# comment\n.codex/config.toml\n",
        )
        .unwrap();
        ensure_entries(wt.path(), &[".codex/config.toml"]).unwrap();
        let s = std::fs::read_to_string(wt.path().join(".gitignore")).unwrap();
        assert_eq!(s.matches(".codex/config.toml").count(), 1);
    }
}
