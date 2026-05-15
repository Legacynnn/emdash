# 0020: File watching — `notify` 6.x + debouncer + ENOSPC fallback

## Status

Accepted

## Context

EMD-11 bundles file-watching and MCP orchestration. The MCP half
requires per-agent adapter ports (Claude / Cursor / Codex / Cline)
and is independent of the watcher infrastructure. **This PR scopes
to file-watching only.** MCP orchestration moves to a follow-up
issue per the EMD-11 spec ("If the PR feels too large, MCP can
split out into its own follow-up").

Three forces shaped the watcher decisions:

- **macOS FSEvents has churned across `notify` versions.** Both
  6.x and 7.x have known platform quirks. We pin to a specific
  6.x patch (`=6.1.1`) and lock the debouncer pair at
  `notify-debouncer-full =0.3.2`, matching Helmor's choice.
- **Linux inotify can ENOSPC.** Recursive watches on a large
  workspace blow through the default per-user watch limit
  (`fs.inotify.max_user_watches = 524288` on most distros, but
  some ship with 8192). The user-visible behavior has to degrade
  gracefully without breaking the renderer.
- **Polling is universally worse than scoped watching.** A
  fallback to polling would surprise users with CPU usage spikes
  in projects with many files. Better to surface a clear "watching
  is degraded; here's the fix" toast than to silently chew CPU.

## Decision

### Domain layer

`src/fs_watcher/` contains:

- `WatchEvent` (typed enum: `Created | Modified | Deleted |
  Renamed { from, to }`). Single `paths: Vec<String>` field on the
  envelope holds the coalesced paths for everything *except*
  renames, which use the inline pair on the variant.
- `WatcherFallback` (`None | InotifyEnospcDepthOne`). Returned
  from `watch(...)` so the caller can decide whether to surface
  the toast.
- `WatcherRegistry` — `parking_lot::Mutex<HashMap<id, WatcherHandle>>`.
  `watch(id, path, listener)` registers (and replaces any prior
  entry under that id); `unwatch(id)` drops it; `is_watching` /
  `watched_count` for introspection.
- `WatcherHandle` owns the `Debouncer` instance; dropping the
  handle stops the watcher thread cleanly.

### Debouncing

`notify-debouncer-full` with a **50 ms timeout**. The 50–100 ms
window in the spec balances "renderer sees updates promptly" against
"don't repaint per individual write." 50 ms felt right after
spot-checking the Helmor pair under a heavy save burst.

### ENOSPC fallback

`watch(...Recursive)` first. On `notify::ErrorKind::Io` with
`raw_os_error() == ENOSPC (28)`, retry with
`RecursiveMode::NonRecursive` and record
`WatcherFallback::InotifyEnospcDepthOne`. The renderer reads the
fallback off the command return and surfaces a one-time toast:

> Recursive file watching is disabled (Linux inotify limit reached).
> Run `sudo sysctl fs.inotify.max_user_watches=524288` to enable
> full coverage.

We **do not** fall back to polling. Documented in this ADR and the
toast copy.

### macOS path-casing

FSEvents reports path with the casing the OS used at the watch
target, which can differ from the canonical case in the
filesystem (case-insensitive HFS+/APFS). Tests under the registry
exercise typical write/create flows on a TempDir; real-world
casing surprises are best handled at the consumer (search,
projects) where the canonical path is already known.

### Tauri glue

`commands::fs_watcher` adapts the runtime-free `EventListener`
callback into `tauri::ipc::Channel<WatchEvent>`. Two commands:

- `fs_watcher_subscribe(id, path, on_event) -> WatcherFallback`
- `fs_watcher_unsubscribe(id) -> bool`

Mirrors the EMD-7 `subscribe_ui_mutations` pattern.

## Out of scope (follow-up issue)

- **MCP orchestration**: per-agent config readers (Claude /
  Cursor / Codex / Cline / others as discovered in the Electron
  build), the canonical config shape, the per-workspace write-lock,
  and the `mcp.list / update / delete` commands. Split into
  **EMD-XX: MCP orchestration** (file before closing EMD-11).
- **Git refs watching**: per-project watcher on `.git/refs`,
  `.git/HEAD`, `.git/config`. The infrastructure ships in this
  PR; wiring the projects module to register a watcher per project
  comes with the next renderer feature that needs git-state freshness.
- **Search-index invalidation**: same — depends on the search port
  (ADR-0015, follow-up issue).

## Consequences

### Easier

- Any subsystem that wants file events (search, git status, the
  renderer's project tree) has a typed, debounced channel ready
  to use.
- The ENOSPC strategy is documented before users hit it, not after.

### Harder

- We commit to maintaining a `notify` 6.x pin. Any future security
  advisory in 6.x requires either a backport or a deliberate jump
  to 7.x with macOS regression testing. Acceptable cost given the
  cross-platform stability premium.
- The watcher registry doesn't yet auto-attach to project create /
  delete events. A renderer that wants per-project watching has
  to call `fs_watcher_subscribe` explicitly until that feature
  ships.
