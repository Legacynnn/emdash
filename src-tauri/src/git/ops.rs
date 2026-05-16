//! Read-only `git2` wrappers used by tasks and (later) the renderer's
//! git inspection UI. Each call opens a `Repository` afresh so callers
//! don't need to share handles across threads — `git2::Repository`
//! isn't `Send`.

use std::path::Path;

use git2::{BranchType, Repository, Status, StatusOptions};
use serde::Serialize;
use specta::Type;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("not a git repository: {0}")]
    NotARepository(String),
    #[error("git2 error: {0}")]
    Git2(#[from] git2::Error),
    #[error("ref not found: {0}")]
    RefNotFound(String),
    #[error("object not found: {0}")]
    ObjectNotFound(String),
}

#[derive(Clone, Debug, Serialize, Type, PartialEq, Eq)]
pub struct GitStatusEntry {
    pub path: String,
    pub bits: u32,
}

#[derive(Clone, Debug, Serialize, Type, PartialEq, Eq)]
pub struct GitRef {
    pub name: String,
    pub kind: RefKind,
    pub target: String,
}

#[derive(Clone, Debug, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    Branch,
    Tag,
    Remote,
    Other,
}

fn open(path: &Path) -> Result<Repository, GitError> {
    Repository::open(path).map_err(|e| match e.code() {
        git2::ErrorCode::NotFound => GitError::NotARepository(path.display().to_string()),
        _ => GitError::Git2(e),
    })
}

pub fn current_branch(path: &Path) -> Result<Option<String>, GitError> {
    let repo = open(path)?;
    let head = match repo.head() {
        Ok(h) => h,
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => return Ok(None),
        Err(e) => return Err(GitError::Git2(e)),
    };
    Ok(head.shorthand().map(|s| s.to_string()))
}

/// Porcelain-style `git status` projection. Returns one entry per
/// changed path. The `bits` value is the raw `git2::Status` bitmask —
/// keep it stable so the renderer can decode without re-implementing
/// the bitset's name table here.
pub fn status_porcelain(path: &Path) -> Result<Vec<GitStatusEntry>, GitError> {
    let repo = open(path)?;
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo.statuses(Some(&mut opts))?;
    let mut out = Vec::with_capacity(statuses.len());
    for entry in statuses.iter() {
        let p = match entry.path() {
            Some(s) => s.to_string(),
            None => continue,
        };
        let bits = entry.status().bits();
        // Skip clean rows; libgit2 occasionally returns Status::CURRENT.
        if Status::from_bits_truncate(bits).is_empty() {
            continue;
        }
        out.push(GitStatusEntry { path: p, bits });
    }
    Ok(out)
}

pub fn diff_against_head(path: &Path) -> Result<String, GitError> {
    let repo = open(path)?;
    let head_tree = match repo.head() {
        Ok(h) => h.peel_to_tree().ok(),
        Err(_) => None,
    };
    let diff = repo.diff_tree_to_workdir_with_index(head_tree.as_ref(), None)?;
    let mut buf = String::new();
    diff.print(git2::DiffFormat::Patch, |_, _, line| {
        // Reproduces what `git diff` writes: a per-line origin marker
        // followed by the content. Sufficient for renderer display
        // until we wire a structured diff model.
        let origin = line.origin();
        if matches!(origin, '+' | '-' | ' ') {
            buf.push(origin);
        }
        if let Ok(s) = std::str::from_utf8(line.content()) {
            buf.push_str(s);
        }
        true
    })?;
    Ok(buf)
}

pub fn list_branches(path: &Path) -> Result<Vec<String>, GitError> {
    let repo = open(path)?;
    let mut out = Vec::new();
    for entry in repo.branches(Some(BranchType::Local))? {
        let (branch, _) = entry?;
        if let Some(name) = branch.name()? {
            out.push(name.to_string());
        }
    }
    out.sort();
    Ok(out)
}

#[derive(Clone, Debug, Serialize, Type, PartialEq, Eq)]
pub struct GitRemote {
    pub name: String,
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Type, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BranchRef {
    Local {
        branch: String,
    },
    Remote {
        branch: String,
        remote: GitRemote,
    },
}

#[derive(Clone, Debug, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalBranchesPayload {
    pub local_branches: Vec<BranchRef>,
    pub current_branch: Option<String>,
    pub is_unborn: bool,
}

#[derive(Clone, Debug, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranchesPayload {
    pub remote_branches: Vec<BranchRef>,
    pub remotes: Vec<GitRemote>,
    pub git_default_branch: String,
}

pub fn local_branches(path: &Path) -> Result<LocalBranchesPayload, GitError> {
    let repo = open(path)?;

    let (current_branch, is_unborn) = match repo.head() {
        Ok(h) => (h.shorthand().map(|s| s.to_string()), false),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
            // libgit2 doesn't expose the unborn HEAD's target name
            // directly; fall back to parsing the HEAD ref.
            let target = repo
                .find_reference("HEAD")
                .ok()
                .and_then(|r| r.symbolic_target().map(|s| s.to_string()))
                .and_then(|s| s.strip_prefix("refs/heads/").map(|s| s.to_string()));
            (target, true)
        }
        Err(e) => return Err(GitError::Git2(e)),
    };

    let mut names = Vec::new();
    for entry in repo.branches(Some(BranchType::Local))? {
        let (branch, _) = entry?;
        if let Some(name) = branch.name()? {
            names.push(name.to_string());
        }
    }
    names.sort();
    let local = names
        .into_iter()
        .map(|branch| BranchRef::Local { branch })
        .collect();

    Ok(LocalBranchesPayload {
        local_branches: local,
        current_branch,
        is_unborn,
    })
}

pub fn remote_branches(path: &Path) -> Result<RemoteBranchesPayload, GitError> {
    let repo = open(path)?;

    let mut remotes: Vec<GitRemote> = Vec::new();
    for name in repo.remotes()?.iter().flatten() {
        if let Ok(r) = repo.find_remote(name) {
            remotes.push(GitRemote {
                name: name.to_string(),
                url: r.url().unwrap_or_default().to_string(),
            });
        }
    }
    remotes.sort_by(|a, b| a.name.cmp(&b.name));

    let mut remote_branches: Vec<BranchRef> = Vec::new();
    for entry in repo.branches(Some(BranchType::Remote))? {
        let (branch, _) = entry?;
        let full = match branch.name()? {
            Some(n) => n.to_string(),
            None => continue,
        };
        // Names from libgit2 look like "origin/main". The remote-tracking
        // pointer for `origin/HEAD` is reported as `origin/HEAD` — skip
        // it so the UI doesn't list a phantom branch.
        let (remote_name, branch_name) = match full.split_once('/') {
            Some((r, b)) => (r.to_string(), b.to_string()),
            None => continue,
        };
        if branch_name == "HEAD" {
            continue;
        }
        let url = remotes
            .iter()
            .find(|r| r.name == remote_name)
            .map(|r| r.url.clone())
            .unwrap_or_default();
        remote_branches.push(BranchRef::Remote {
            branch: branch_name,
            remote: GitRemote {
                name: remote_name,
                url,
            },
        });
    }
    remote_branches.sort_by(|a, b| match (a, b) {
        (
            BranchRef::Remote {
                branch: ab,
                remote: ar,
            },
            BranchRef::Remote {
                branch: bb,
                remote: br,
            },
        ) => ar.name.cmp(&br.name).then_with(|| ab.cmp(bb)),
        _ => std::cmp::Ordering::Equal,
    });

    // Best-effort: derive the default branch from origin/HEAD if present,
    // otherwise leave empty (renderer falls back to its own resolver).
    let git_default_branch = repo
        .find_reference("refs/remotes/origin/HEAD")
        .ok()
        .and_then(|r| r.symbolic_target().map(|s| s.to_string()))
        .and_then(|s| s.strip_prefix("refs/remotes/origin/").map(|s| s.to_string()))
        .unwrap_or_default();

    Ok(RemoteBranchesPayload {
        remote_branches,
        remotes,
        git_default_branch,
    })
}

pub fn list_refs(path: &Path) -> Result<Vec<GitRef>, GitError> {
    let repo = open(path)?;
    let mut out = Vec::new();
    for reference in repo.references()? {
        let reference = reference?;
        let name = match reference.name() {
            Some(n) => n.to_string(),
            None => continue,
        };
        let kind = if reference.is_branch() {
            RefKind::Branch
        } else if reference.is_tag() {
            RefKind::Tag
        } else if reference.is_remote() {
            RefKind::Remote
        } else {
            RefKind::Other
        };
        let target = reference
            .target()
            .map(|oid| oid.to_string())
            .or_else(|| reference.symbolic_target().map(|s| s.to_string()))
            .unwrap_or_default();
        out.push(GitRef { name, kind, target });
    }
    Ok(out)
}

pub fn branch_head(path: &Path, branch: &str) -> Result<String, GitError> {
    let repo = open(path)?;
    let reference = repo
        .find_branch(branch, BranchType::Local)
        .map_err(|e| match e.code() {
            git2::ErrorCode::NotFound => GitError::RefNotFound(branch.to_string()),
            _ => GitError::Git2(e),
        })?;
    reference
        .get()
        .target()
        .map(|oid| oid.to_string())
        .ok_or_else(|| GitError::RefNotFound(branch.to_string()))
}

pub fn commit_message(path: &Path, oid_str: &str) -> Result<String, GitError> {
    let repo = open(path)?;
    let oid =
        git2::Oid::from_str(oid_str).map_err(|_| GitError::ObjectNotFound(oid_str.to_string()))?;
    let commit = repo
        .find_commit(oid)
        .map_err(|_| GitError::ObjectNotFound(oid_str.to_string()))?;
    Ok(commit.message().unwrap_or_default().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn init_repo() -> (TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        run(&path, &["init", "-q"]);
        run(&path, &["config", "user.email", "test@example.com"]);
        run(&path, &["config", "user.name", "Test"]);
        run(&path, &["commit", "--allow-empty", "-m", "init"]);
        // git init defaults to master on older platforms; pin to main.
        run(&path, &["branch", "-M", "main"]);
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
    fn current_branch_returns_main_after_init() {
        let (_dir, path) = init_repo();
        let branch = current_branch(&path).unwrap();
        assert_eq!(branch.as_deref(), Some("main"));
    }

    #[test]
    fn status_reports_untracked_file() {
        let (_dir, path) = init_repo();
        std::fs::write(path.join("a.txt"), "hi").unwrap();
        let entries = status_porcelain(&path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "a.txt");
        assert!(Status::from_bits_truncate(entries[0].bits).is_wt_new());
    }

    #[test]
    fn list_branches_returns_initial_branch() {
        let (_dir, path) = init_repo();
        let branches = list_branches(&path).unwrap();
        assert_eq!(branches, vec!["main".to_string()]);
    }

    #[test]
    fn branch_head_returns_oid() {
        let (_dir, path) = init_repo();
        let oid = branch_head(&path, "main").unwrap();
        assert_eq!(oid.len(), 40);
    }

    #[test]
    fn open_on_non_repo_returns_not_a_repository() {
        let dir = tempfile::tempdir().unwrap();
        let err = current_branch(dir.path()).unwrap_err();
        assert!(matches!(err, GitError::NotARepository(_)));
    }

    #[test]
    fn diff_against_head_reports_modified_file() {
        let (_dir, path) = init_repo();
        std::fs::write(path.join("a.txt"), "hi\n").unwrap();
        run(&path, &["add", "a.txt"]);
        run(&path, &["commit", "-m", "add a"]);
        std::fs::write(path.join("a.txt"), "bye\n").unwrap();
        let diff = diff_against_head(&path).unwrap();
        assert!(diff.contains("-hi"));
        assert!(diff.contains("+bye"));
    }

    #[test]
    fn local_branches_returns_payload_with_current_branch() {
        let (_dir, path) = init_repo();
        run(&path, &["branch", "feature/a"]);
        run(&path, &["branch", "feature/b"]);

        let payload = local_branches(&path).unwrap();
        assert_eq!(payload.current_branch.as_deref(), Some("main"));
        assert!(!payload.is_unborn);
        assert_eq!(payload.local_branches.len(), 3);
        let names: Vec<&str> = payload
            .local_branches
            .iter()
            .map(|b| match b {
                BranchRef::Local { branch } => branch.as_str(),
                _ => unreachable!("local_branches must only contain Local"),
            })
            .collect();
        assert_eq!(names, vec!["feature/a", "feature/b", "main"]);
    }

    #[test]
    fn local_branches_reports_unborn_for_fresh_repo() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        run(&path, &["init", "-q", "-b", "main"]);

        let payload = local_branches(&path).unwrap();
        assert!(payload.is_unborn);
        assert_eq!(payload.current_branch.as_deref(), Some("main"));
        assert!(payload.local_branches.is_empty());
    }

    #[test]
    fn remote_branches_lists_origin_branches() {
        let (_origin_dir, origin) = init_repo();
        run(&origin, &["branch", "feature/x"]);

        let clone_dir = tempfile::tempdir().unwrap();
        let clone_path = clone_dir.path().join("clone");
        let out = Command::new("git")
            .args([
                "clone",
                "-q",
                origin.to_str().unwrap(),
                clone_path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(out.status.success());

        let payload = remote_branches(&clone_path).unwrap();
        assert_eq!(payload.remotes.len(), 1);
        assert_eq!(payload.remotes[0].name, "origin");

        let names: Vec<&str> = payload
            .remote_branches
            .iter()
            .map(|b| match b {
                BranchRef::Remote { branch, .. } => branch.as_str(),
                _ => unreachable!("remote_branches must only contain Remote"),
            })
            .collect();
        assert!(names.contains(&"main"));
        assert!(names.contains(&"feature/x"));
        // The synthetic origin/HEAD pointer must not surface as a branch.
        assert!(!names.contains(&"HEAD"));
    }
}
