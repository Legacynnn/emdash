
---
default_branch: dev
package_manager: pnpm
node_version: "24.x.x"
start_command: "pnpm run d"
dev_command: "pnpm run dev"
build_command: "pnpm run build"
test_commands:
  - "pnpm run format"
  - "pnpm run lint"
  - "pnpm run typecheck"
  - "pnpm run test"
ports:
  dev: 1420
required_env: []
optional_env:
  - TELEMETRY_ENABLED
  - EMDASH_DB_FILE
  - EMDASH_DISABLE_NATIVE_DB
  - EMDASH_DISABLE_CLONE_CACHE
  - EMDASH_DISABLE_PTY
  - CODEX_SANDBOX_MODE
  - CODEX_APPROVAL_POLICY
---

# Emdash Agent Guide

Start here. Load only the linked `agents/` docs that are relevant to the task.

## Start Here

- Repo map: `agents/README.md`
- Setup and commands: `agents/quickstart.md`
- System overview: `agents/architecture/overview.md`
- Validation flow: `agents/workflows/testing.md`

## Read By Task

- Main-process changes: `agents/architecture/main-process.md`
- Renderer/UI changes: `agents/architecture/renderer.md`
- Shared types or provider metadata: `agents/architecture/shared.md`
- Worktree behavior or `.emdash.json`: `agents/workflows/worktrees.md`
- SSH or remote project work: `agents/workflows/remote-development.md`
- Provider integration or CLI behavior: `agents/integrations/providers.md`
- MCP changes: `agents/integrations/mcp.md`

## High-Risk Areas

- Database and migrations: `agents/risky-areas/database.md`
- PTY/session orchestration: `agents/risky-areas/pty.md`
- SSH and shell escaping: `agents/risky-areas/ssh.md`
- Auto-update and packaging: `agents/risky-areas/updater.md`

## Conventions

- IPC contract and typing: `agents/conventions/ipc.md`
- Main process patterns (controllers, services, Result type, events): `agents/conventions/main-patterns.md`
- Renderer patterns (modals, views, PTY frontend, React Query contexts): `agents/conventions/renderer-patterns.md`
- TypeScript and React norms: `agents/conventions/typescript.md`
- Config files and repo rules: `agents/conventions/config-files.md`
- Never do re exports always import from the original source

### State Guard Conventions (renderer stores)

`ProjectStore` and `TaskStore` are mutable MobX class instances that transition through states. Use the following layers — do not mix them:

**Selectors** (`task-selectors.ts`, `project-selectors.ts`) — pure functions, safe in observer components, effects, and event handlers:
- `getTaskStore(projectId, taskId)` → `TaskStore | undefined`
- `asProvisioned(store)` → `ProvisionedTask | undefined` (use with explicit null check, never `!`)
- `taskViewKind(store, projectId)` → `TaskViewKind`
- `getTaskManagerStore(projectId)` → `TaskManagerStore | undefined` (use this instead of reaching through project store)
- `getProjectStore(projectId)` → `ProjectStore | undefined`
- `asMounted(store)` → `MountedProject | undefined` (use with explicit null check, never `!`)

**Hooks** (`task-view-context.tsx`) — for `observer` components inside the task view tree:
- `useTaskViewKind()` — routing/state-gating
- `useProvisionedTask()` → `ProvisionedTask | null` — when the component handles a non-provisioned state
- `useRequireProvisionedTask()` → `ProvisionedTask` — when the component must only render when provisioned (throws with a descriptive error if the invariant is violated)

**Rules:**
- Never `asProvisioned(...)!` or `asMounted(...)!` — use the hook or an explicit null check
- State guards must use `kind !== 'ready'`, never enumerate non-ready states (new states would silently fall through)
- Access task manager via `getTaskManagerStore(projectId)`, not through `project.taskManager`
- Access mounted project via `asMounted(getProjectStore(id))`, not via inline `isMountedProject` guards

## Tauri 2 + Rust — the only build

The app is a Tauri 2 + Rust desktop app. `pnpm run dev` /
`pnpm run build` / `pnpm run package` all target Tauri. The Electron
tree was removed; see `docs/migration/tauri-renderer-wire.md` for what
was ported and where the residual stubs live. Conventions and decisions
for `src-tauri/` live in `docs/decisions/` (Michael Nygard ADRs).

The React UI source still lives at `src/renderer/` (shared layout
inherited from the migration). The Tauri UI shell at `src-tauri/ui/`
dynamically imports it and installs a `window.electronAPI`-shaped
polyfill backed by Tauri invoke + UiMutationEvent channels — the shim
implementation is in `src-tauri/ui/src/shim/`. Channels routed
through `route-table.ts` cover the renderer's full RPC surface; the
remaining `STATIC_*` entries return sensible empty / not-supported
envelopes for surfaces whose Rust implementation is deferred.

Rules specific to `src-tauri/`:

- **Domain code in `lib.rs` must stay free of `tauri::AppHandle`.** Modules
  reachable through `pub mod` from `lib.rs` (`shell_env`, `greeting`, etc.)
  must not use `#[tauri::command]` annotations or import/accept
  `tauri::AppHandle`, `tauri::Window`, or any webview-runtime type.
  `src-tauri/tests/domain_boundaries.rs` enforces this with a source-level
  guard. The near-empty `src-tauri/src/bin/emdash-cli.rs` exists to keep the
  library modules compiling from a non-webview entry point.
- **Every command must be allowlisted.** `src-tauri/allowed-commands.json`
  enumerates the channels exposed through `invoke_handler`. `build.rs`
  fails the build if `ui/src/bindings.ts` and the allowlist drift. To add
  a command: write it under `src/commands/`, append to
  `collect_commands![]` in `app.rs`, run
  `cargo run --bin emdash-dev -- --export-bindings`, then add the channel
  name to `allowed-commands.json`.
- **`ui/src/bindings.ts` is generated and committed.** Do not hand-edit it.
  Regenerate via the `--export-bindings` flag above (debug builds also
  regenerate on startup). Wire-format changes are pinned by an `insta`
  snapshot in `tests/wire_format.rs`; deliberate changes need
  `cargo insta accept`.
- **Single `UiMutationEvent` bridge** (EMD-7 / ADR-0004). Renderer cache
  invalidation flows through one `Channel<UiMutationEvent>` opened by
  `src-tauri/ui/src/ui-sync/useUiMutations.ts`. Host code calls
  `UiSyncManager::broadcast(...)`; never `app.emit(...)`. The renderer
  routes events through one `switch` in `ui-sync/dispatch.ts`. The
  `eslint-plugin-emdash` rule `no-tauri-event-bus` enforces this at
  lint time (CI fails on violation). Exemption pragma:
  `// emdash-disable-next-line no-tauri-event-bus -- <reason>` — the
  only sanctioned exemption today is the `Channel<UiMutationEvent>`
  construction inside `useUiMutations.ts` itself.
- **How to add a new feature** (the EMD-7 template). For each feature:
  1. Add a domain module under `src-tauri/src/<feature>/` (no Tauri
     imports — `tests/domain_boundaries.rs` enforces this).
  2. Add Tauri glue in `src-tauri/src/commands/<feature>.rs`. Map domain
     errors to a `{code, message}` envelope; broadcast a
     `UiMutationEvent` variant after every successful write.
  3. Append commands to `collect_commands![]` in `tauri_bindings.rs`,
     run `cargo run --bin emdash-dev -- --export-bindings`, then add
     the channel names to `allowed-commands.json`.
  4. Add an `insta` snapshot per command and per new `UiMutationEvent`
     variant in `src-tauri/tests/wire_format.rs`.
  5. On the renderer side: bindings → MobX store →
     `ui-sync/dispatch.ts` arm → observer component. Add a Vitest
     smoke test that mocks `@tauri-apps/api/core`.
  Reference implementation: the `projects` end-to-end from EMD-7.
- **Versions are pinned exactly.** `tauri`, `tauri-build`, `tauri-specta`,
  `specta`, and `specta-typescript` are pinned with `=`. Bumping any of
  them is a deliberate PR; see ADR-0001 for the reasoning.

Build/test commands for `src-tauri/`:

```bash
cd src-tauri
cargo check                         # type-check + run capability allowlist
cargo test                          # unit + wire-format snapshot tests
cargo run --bin emdash-cli -- -V    # invariant: CLI compiles without webview
cargo run --bin emdash-dev -- --export-bindings   # regenerate bindings.ts
```

`cargo tauri dev` requires the Tauri CLI: `cargo install tauri-cli --locked --version "^2"`.

**pnpm workspace note.** `src-tauri/ui` is a pnpm workspace member, so a plain
`pnpm install` at the repo root resolves React + Vite for it alongside the
Electron deps. If you're only working on Electron and want to skip the Tauri
UI tree, pass `--filter '!emdash-dev-ui'`. CI uses
`--filter emdash-dev-ui...` to install only the Tauri UI subgraph for the
emdash-dev build job.

## Non-Negotiables

- Run `pnpm run format`, `pnpm run lint`, `pnpm run typecheck`, and `pnpm test` before merging.
- New Tauri commands go under `src-tauri/src/commands/<feature>.rs`, get registered in `collect_commands![]` (`src-tauri/src/tauri_bindings.rs`), and appended to `src-tauri/allowed-commands.json`. Regenerate `ui/src/bindings.ts` via `cargo run --bin emdash-dev -- --export-bindings`.
- New renderer-side RPC routes go in `src-tauri/ui/src/shim/route-table.ts` (one entry per `namespace.method`); shim type changes go in `src-tauri/ui/src/shim/electron-api.ts`.
- New modals must be registered in `src/renderer/core/modal/registry.ts`.
- New views must be registered in `src/renderer/core/view/registry.ts`.
- Treat `src-tauri/src/pty/`, `src-tauri/src/agents/`, `src-tauri/src/db/`, and updater code as high risk.
- The docs app in `docs/` is separate from the Tauri renderer and defaults to port `3000`; the Tauri dev server uses port `1420`.
