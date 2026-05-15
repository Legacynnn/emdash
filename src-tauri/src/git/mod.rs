//! Read-heavy git operations via `git2` (libgit2 bindings).
//!
//! **Scope:** status, log, diff, refs, `cat_file`. Worktree
//! create/destroy is *not* here — that path shells out to the `git`
//! CLI (see `tasks::worktree`) because libgit2's worktree API is
//! finicky around locked/missing worktrees and doesn't always match
//! CLI semantics.
//!
//! Tauri-runtime-free: no `tauri::*` types appear in this module.
//! Domain-boundary tests enforce that.

pub mod ops;

pub use ops::{
    branch_head, commit_message, current_branch, diff_against_head, list_branches, list_refs,
    status_porcelain, GitError, GitRef, GitStatusEntry, RefKind,
};
