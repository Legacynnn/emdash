# Tauri Renderer Wiring — Migration Plan

This doc tracks the systematic port of the Electron renderer onto the
Tauri 2 + Rust host so the Electron tree can be deleted.

## Current state (as of EMD-16+)

The Tauri UI shell (`src-tauri/ui/`) dynamically imports the Electron
renderer (`src/renderer/main.tsx`) and installs a `window.electronAPI`
polyfill backed by Tauri invoke + UiMutationEvent channels. The
polyfill (`src-tauri/ui/src/shim/electron-api.ts`) routes every
Electron RPC channel through a table (`route-table.ts`) that either:

1. **invokes** a real Tauri command,
2. resolves a **custom** handler (e.g. JSON-stringify for view state), or
3. returns a **static** stub (typed-correctly empty value so the
   renderer doesn't crash).

`pnpm run dev` now runs `cargo tauri dev`. Electron scripts are kept
under `:electron` suffixes during the migration. The Electron renderer
source under `src/renderer/` is shared with the Tauri build during this
window — once every channel is backed by a real command, the renderer
moves under `src-tauri/ui/src/` and the Electron tree can be deleted.

## Ported (real implementations)

| Namespace | Status | Notes |
|-----------|--------|-------|
| `projects` (CRUD) | done | `projects_list / add / remove` + shape adapter |
| `tasks` (create/delete) | done | `tasks_list / create / delete` with worktree |
| `viewState` | done | JSON-encoded on the host; shim parses on read |
| `telemetry` (get/set) | done | Native opt-in toggle |
| `update.check` | done | `tauri-plugin-updater` |
| `github` (sign-in, repos, PRs) | partial | Status/logout wired; clone/init still stubs |
| `linear` (saveToken / clearToken) | partial | Token-only |
| `agents` (start/stop/list) | done | EMD-27 local invocation |
| `editorBuffer` | done | Save/clear/list |
| `app.*` (version, platform, open, etc.) | done | Shell out to OS handlers |
| `fs.*` (read/write/list/image) | partial | Rust commands ship; shim still stubbed pending workspace-path resolver |
| `workspaces.resolveBootstrap` | done | Tauri tasks are always `{ kind: 'ready' }` |

## Outstanding ports (route-table is `STATIC_*`)

Order is rough priority — earlier work unblocks more of the renderer.

### Tier 1 — required for core flows

- **PTY** (`pty.subscribe / unsubscribe / sendInput / resize / kill /
  uploadFiles`). The renderer uses string `sessionId`s
  (`projectId:scopeId:leafId`) and a ring-buffer subscribe model;
  Tauri exposes opaque `PtyId`s and per-spawn `Channel<Vec<u8>>`. The
  shim needs to maintain a `sessionId → PtyId` map and translate
  channel bytes into `pty:data.<sessionId>` events. Spawn flow
  belongs in `agents_start` so the renderer's pty.subscribe really
  attaches to an already-running agent.
- **Conversations + terminals** (`getConversations`,
  `getConversationsForTask`, CRUD, `renameConversation`,
  `touchConversation`). Need DB tables + commands. The renderer
  attaches PTY sessions to conversation/terminal records.
- **Tasks completeness** (`archiveTask`, `restoreTask`, `renameTask`,
  `generateTaskName`, `provisionTask`, `setTaskPinned`,
  `updateTaskStatus`, `updateLinkedIssue`, `getWorkspaceSettings`).
  Some need schema migrations (pinned, archived status, linked_issue
  column).
- **FS workspace resolution** — port a `fs_*_in_workspace(project_id,
  workspace_id, rel_path, ...)` family in Rust that resolves to an
  absolute path via the tasks table, then wire the renderer's
  fs.* calls through those instead of the absolute-path commands
  added in this PR.
- **Git read** (`getFullStatus`, `getChangedFiles`, `getLog`,
  `getFileAtRef / Index`, `getImageAtRef / Index`). Tauri already
  has `git2` vendored; expose porcelain wrappers.
- **Git write** (`commit`, `push`, `pull`, `publishBranch`, stage /
  unstage / revert).
- **Repository** (`fetch`, `addRemote`, `getLocalBranches`,
  `getRemoteBranches`).

### Tier 2 — required for full functionality

- **App settings** (`appSettings.*`) — a small KV store similar to
  `view_state` but per-app, with feature-flag overrides.
- **Provider settings** (`providerSettings.*`) — namespaced settings
  per provider (GitHub, Linear, etc.).
- **Resource monitor** (`resourceMonitor.getSnapshot`) — system CPU
  + per-agent samples. Renderer expects `{ ok: true, value: {
  cpuCount, app, entries } }`.
- **FS watcher events** — extend `fs_watcher` to emit events through
  the `UiMutationEvent` channel (or a dedicated channel) and have the
  shim re-emit them as `fs:watch-event.<projectId>:<workspaceId>`.
- **Update** (`update.download`, `quitAndInstall`, `getReleaseNotes`,
  `getState`). Plugin is wired; commands need to bridge through.
- **Dependencies** (`dependencies.getAll / probeAll / install /
  probeCategory`). Probe agent CLIs on PATH.

### Tier 3 — degrade-gracefully candidates

These can keep their static stubs for a longer window — the renderer
shows "signed out / empty / unavailable" states which is acceptable
while we focus on Tier 1+2:

- **Pull requests** (`pullRequests.*`) — 14 methods. Partially backed
  by `github_list_pulls` already.
- **Skills** (`skills.*`) — 6 methods. Renderer surfaces a catalog;
  empty is fine.
- **MCP** (`mcp.*`) — 5 methods. Same.
- **Issues / provider credentials** (forgejo, gitlab, jira, plain,
  featurebase). Most just store a token via the secrets layer.
- **SSH** (`ssh.*`) — large surface, very high risk. Worth keeping as
  stub until a dedicated PR.
- **Search** (`search.commandPalette`) — global command search.
- **Account** (`account.*`) — cloud sync; not strictly required.

### Final cleanup

1. Move `src/renderer/` under `src-tauri/ui/src/renderer/`.
2. Switch `rpc.*` from `window.electronAPI.invoke` to direct
   `@tauri-apps/api/core invoke`, then delete the shim.
3. Delete `src/main/`, `src/preload/`, `out/`, Electron build configs.
4. Remove `electron`, `electron-vite`, `electron-builder`, `node-pty`,
   `better-sqlite3` from package.json.
5. Update `pnpm test`, lint, typecheck targets to point at the new
   layout.

## How to add a port

1. Write the domain code under `src-tauri/src/<feature>/` (no Tauri
   imports — `tests/domain_boundaries.rs` enforces this).
2. Add Tauri glue under `src-tauri/src/commands/<feature>.rs`. Map
   domain errors to a `{ code, message }` envelope.
3. Broadcast a `UiMutationEvent` variant after every successful write
   so renderer caches invalidate.
4. Register the command in `tauri_bindings.rs` `collect_commands![]`.
5. Run `cargo run --bin emdash-dev -- --export-bindings` to regenerate
   `ui/src/bindings.ts`.
6. Append the command name to `allowed-commands.json`.
7. Update the route in `src-tauri/ui/src/shim/route-table.ts` —
   replace `STATIC_*` with `{ kind: 'invoke', command, adapt }` (and a
   `transform` if the renderer expects a different shape).
8. Add an `insta` snapshot per new command + UiMutationEvent variant
   in `src-tauri/tests/wire_format.rs`.

Reference implementations:
- Projects end-to-end (EMD-7): `src/projects/`, `commands/projects.rs`,
  route in `route-table.ts`.
- App + fs utility commands: `commands/app.rs`, `commands/fs.rs`.
- Renderer-foundation shim: `src-tauri/ui/src/shim/`.
