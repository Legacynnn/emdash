# 0005: Git worktrees and the tasks table

## Status

Accepted

## Context

EMD-17 introduces the first stateful per-task primitive: one task = one
git worktree under the host project, plus a row in the `tasks` table
that owns its lifecycle. Three forces drove the specific choices below:

- **`git2` vs shelling out.** `git2` is fast and structured for the
  read-heavy paths (status, log, diff, refs, object reads) but
  libgit2's worktree API is notoriously finicky around locked or
  missing worktrees and doesn't always match `git worktree` CLI
  semantics (one example: `worktree remove --force` succeeds on a
  dirty tree where libgit2 returns an error). Users running emdash-dev
  alongside the `git` CLI deserve identical worktree behavior across
  the two.
- **Schema growth on a single bootstrap.** EMD-6 ships the database
  as one collapsed bootstrap migration that mirrors the Electron
  final-state schema. EMD-17 needs two columns the Electron schema
  doesn't carry (`path`, `pty_id`) and treats `source_branch` as JSON
  from day one. Adding the columns to the bootstrap is cheaper than
  introducing a second migration that the user (with no upgrade path
  from Electron) would never run.
- **Concurrent worktree mutations.** Two simultaneous deletes on the
  same task could leave a half-deleted worktree on disk and a stale
  row in the DB. Helmor uses a per-workspace `Arc<Mutex<()>>` map to
  serialize the FS+DB pair as one unit; the Tauri side adopts the
  same primitive.

## Decision

- **`git2` for read ops only.** A thin `git` domain module exposes
  `current_branch`, `status_porcelain`, `diff_against_head`,
  `list_branches`, `list_refs`, `branch_head`, `commit_message`. Each
  call re-opens the `Repository` (libgit2 handles aren't `Send`) — the
  ergonomic cost is small relative to the threading flexibility it
  buys.
- **Shell out for `git worktree`.** `tasks::worktree::add(...)` /
  `remove(...)` invoke the user's `git` binary with `current_dir =
  project_root`, so `.gitconfig` and the user's PATH-resolved git
  version both apply. Output is treated as opaque; failures return a
  `TasksError::WorktreeFailed(stderr)`.
- **`source_branch` as a JSON discriminator.**
  `{"type":"local","branch":"..."}` or
  `{"type":"remote","host":"...","branch":"..."}`, stored in a `text`
  column. `TaskSourceBranch::checkout_target` translates to the string
  `git worktree add ... <ref>` consumes (`branch` for local,
  `host/branch` for remote — matches what `git fetch` writes under
  `refs/remotes/`).
- **`WorkspaceFsMutationLock`.** A `parking_lot::Mutex<HashMap<String,
  Arc<Mutex<()>>>>` map keyed by workspace id (in v1, the task id).
  `lock_for(id)` returns a cheap `Arc` clone; callers `.lock()` and
  hold the guard across the FS+DB transaction. Entries are never
  evicted — keeping the map monotonically growing avoids a TOCTOU
  between "I asked for the lock and someone else deleted the entry."
- **Bootstrap schema extension.** The `tasks` table in
  `db::migrations::BOOTSTRAP_SQL` now includes `path text NOT NULL`
  and `pty_id text`, plus an index on `pty_id` for the EMD-27
  "find-task-by-pty" lookup. All other columns from the Electron
  schema remain so the schema diff stays minimal and migration-free.
- **Worktree path layout.** `<project_root>/.emdash-worktrees/<task_id>/`.
  Hidden by leading-dot convention; the segment is the UUID-v4 task
  id, which guarantees no collision between sibling tasks. On Windows
  the path arg is wrapped with the `\\?\` long-path prefix at the
  shell-out site.
- **Renderer wiring.** `taskStore` (MobX) mirrors `projectStore`. The
  store map (`tasksByProject`) lives on `RendererStores` and is
  non-observable — only the individual `TaskStore` instances are.
  `TaskCreated`/`TaskUpdated` events trigger a `tasksByProject.get(...)?.load()`;
  `TaskDeleted` calls `applyDeleted(id)` for an immediate UI update.

## Consequences

### Easier

- The CLI escape hatch is one process call away: if libgit2 ever
  regresses on a worktree edge case, we don't have to swap a domain
  module — the worktree path is already a shell-out.
- `source_branch` is open for `remote` from day one. EMD-10 (SSH) and
  EMD-11/EMD-13/EMD-14 (providers) can write `{"type":"remote",...}`
  without a migration.
- The FS-mutation lock is reusable: EMD-27 attaches a PTY to an
  existing worktree under the same per-workspace serialization.

### Harder

- Two code paths for git (lib + CLI). The boundary is documented and
  tested per-module; a regression that picks the wrong path would
  show up as either a behaviour drift from the user's CLI or an
  unnecessary subprocess on a hot loop.
- `tasks` table now diverges from the Electron schema by two columns.
  Acceptable — see Context — but worth flagging if a future port
  attempts to share migration history.
- `WorkspaceFsMutationLock` entries never evict. Memory is bounded by
  total tasks the user has ever created in a session; in practice
  this is dozens, not millions. If a workload changes that, switch to
  `weak::Weak<Mutex>` entries with TTL eviction.
