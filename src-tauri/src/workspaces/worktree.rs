//! Worktree create/remove via the `git` CLI.
//!
//! libgit2's worktree API is finicky around locked/missing worktrees
//! and doesn't always match `git worktree` CLI semantics (one common
//! divergence: `worktree remove --force` succeeds on a dirty worktree
//! while libgit2 reports an error). Shelling out keeps user-facing
//! behavior identical to what `git worktree` does in the user's
//! environment.
//!
//! Path layout: `<project_root>/.emdash-worktrees/<branch>/`. The
//! worktree directory mirrors the branch name (including any `/`
//! Conventional-Commits-style prefix, which becomes a subdirectory).
//! That keeps "the branch and the workspace are called the same
//! thing locally and remotely" as a single invariant.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::model::WorkspacesError;

pub const WORKTREES_DIR_NAME: &str = ".emdash-worktrees";

/// Path where the workspace's worktree will live, given the project root
/// and the branch name. Returns an absolute path with the platform's
/// canonical separators. Embedded `/` in the branch (e.g. `feat/x`)
/// turns into a nested subdirectory — git handles that fine.
pub fn worktree_path(project_root: &Path, branch: &str) -> PathBuf {
    project_root.join(WORKTREES_DIR_NAME).join(branch)
}

/// `git worktree add -b <workspace_branch> <path> <source_branch>`.
///
/// `workspace_branch` is the **new** branch the worktree checks out (a
/// workspace-scoped name created on demand). `source_branch` is the
/// starting point, typically the result of
/// `WorkspaceSourceBranch::checkout_target`.
pub fn add(
    project_root: &Path,
    worktree_path: &Path,
    workspace_branch: &str,
    source_branch: &str,
) -> Result<(), WorkspacesError> {
    if let Some(parent) = worktree_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| WorkspacesError::WorktreeFailed(format!("mkdir {}: {e}", parent.display())))?;
    }
    let path_arg = canonical_path_arg(worktree_path);
    let output = Command::new("git")
        .arg("worktree")
        .arg("add")
        .arg("-b")
        .arg(workspace_branch)
        .arg(&path_arg)
        .arg(source_branch)
        .current_dir(project_root)
        .output()
        .map_err(|e| WorkspacesError::WorktreeFailed(format!("spawn git: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(WorkspacesError::WorktreeFailed(format!(
            "git worktree add failed: {stderr}"
        )));
    }
    Ok(())
}

/// `git worktree remove --force <path>`.
///
/// Force is intentional: removing a workspace in the renderer should never
/// fail because the user left a dirty file behind. Lost work would be
/// surprising; the renderer should warn before calling delete.
pub fn remove(project_root: &Path, worktree_path: &Path) -> Result<(), WorkspacesError> {
    let path_arg = canonical_path_arg(worktree_path);
    let output = Command::new("git")
        .arg("worktree")
        .arg("remove")
        .arg("--force")
        .arg(&path_arg)
        .current_dir(project_root)
        .output()
        .map_err(|e| WorkspacesError::WorktreeFailed(format!("spawn git: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(WorkspacesError::WorktreeFailed(format!(
            "git worktree remove failed: {stderr}"
        )));
    }
    // Prune stale records — harmless if the worktree was already clean.
    let _ = Command::new("git")
        .arg("worktree")
        .arg("prune")
        .current_dir(project_root)
        .output();
    Ok(())
}

/// On Windows, the worktree path may exceed MAX_PATH. Prefix with
/// `\\?\` so `git.exe` reaches the long-path code path. On other
/// platforms the path is returned untouched.
#[cfg(windows)]
fn canonical_path_arg(p: &Path) -> std::ffi::OsString {
    let s = p.as_os_str().to_string_lossy().to_string();
    if s.starts_with(r"\\?\") {
        s.into()
    } else if s.starts_with(r"\\") {
        // UNC path; git handles these without the extended prefix.
        s.into()
    } else {
        format!(r"\\?\{s}").into()
    }
}

#[cfg(not(windows))]
fn canonical_path_arg(p: &Path) -> std::ffi::OsString {
    p.as_os_str().to_os_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn init_repo() -> (TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        run(&path, &["init", "-q", "-b", "main"]);
        run(&path, &["config", "user.email", "test@example.com"]);
        run(&path, &["config", "user.name", "Test"]);
        run(&path, &["commit", "--allow-empty", "-m", "init"]);
        (dir, path)
    }

    fn run(repo: &Path, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn worktree_path_under_emdash_worktrees() {
        let p = worktree_path(Path::new("/a/b"), "ws-1");
        assert!(p.ends_with(".emdash-worktrees/ws-1"));
    }

    #[test]
    fn worktree_path_nests_slashes_from_branch() {
        let p = worktree_path(Path::new("/a/b"), "feat/add-search");
        assert!(p.ends_with(".emdash-worktrees/feat/add-search"));
    }

    #[test]
    fn add_creates_directory_with_checkout() {
        let (_dir, project) = init_repo();
        let wt = worktree_path(&project, "ws-abc");
        add(&project, &wt, "ws/abc", "main").unwrap();
        assert!(wt.is_dir(), "worktree dir should exist");
        // git worktree should write a .git pointer file.
        let dotgit = wt.join(".git");
        assert!(dotgit.exists(), ".git pointer file/dir should exist");
    }

    #[test]
    fn add_fails_when_path_already_exists() {
        let (_dir, project) = init_repo();
        let wt = worktree_path(&project, "ws-abc");
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(wt.join("placeholder"), "x").unwrap();
        let err = add(&project, &wt, "ws/abc", "main").unwrap_err();
        assert!(matches!(err, WorkspacesError::WorktreeFailed(_)));
    }

    #[test]
    fn remove_deletes_worktree_dir() {
        let (_dir, project) = init_repo();
        let wt = worktree_path(&project, "ws-x");
        add(&project, &wt, "ws/x", "main").unwrap();
        assert!(wt.is_dir());
        remove(&project, &wt).unwrap();
        assert!(!wt.is_dir(), "worktree dir should be gone after remove");
    }
}
