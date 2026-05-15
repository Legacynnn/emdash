# 0024: Local agent invocation — the three-primitive integration point

## Status

Accepted

## Context

EMD-27 is the integration where the three foundational primitives —
**worktree** (EMD-17), **PTY** (EMD-8), and **hook server**
(EMD-9) — first meet. By the end of v1 a user clicks "Start agent"
on a task, picks a provider, and watches the agent's output stream
into an xterm.js terminal. Agents POST back to our local hook
server using env vars we inject at spawn.

The interesting constraints:

- **Start ordering.** The hook server must be bound (port + token
  decided) before any agent is spawned. Inject the wrong port and
  the agent's POSTs vanish.
- **PTY-per-task concurrency.** Two concurrent "Start agent" clicks
  on the same task can't both spawn — race-y double-PTY would
  desync the `tasks.pty_id` column.
- **Provider config has to evolve without a schema migration.** v1
  ships a hard-coded enum; the user-extensible manifest is a v1.x
  follow-up.

## Decision

### Domain layer — `agents/`

`AgentService` composes:

- `Arc<Db>` — for the `tasks.pty_id` column update on start/stop.
- `Arc<pty::Registry>` — to actually spawn the PTY.
- `Arc<WorkspaceFsMutationLock>` — the same per-task lock EMD-17
  uses for FS+DB serialization. We re-use it so worktree
  create/destroy and agent start/stop can't interleave on the
  same task.
- `hook_port: u16` + `hook_token: String` — captured at construction
  so the `inject_hook_env_into` call doesn't need a `HookServerHandle`
  reference everywhere.

`AgentProvider` is a typed enum (`Codex` / `Claude` / `Cursor` /
`Opencode` / `Continue` / `Cline`). `provider_spec(provider)`
returns a `ProviderSpec { provider, label, binary, args, env }`.
`known_providers()` enumerates them for the renderer's picker.

### Env merge order (load-bearing)

When spawning the agent, env is built in this exact order:

1. Start with `shell_env::shell_env().captured` — the captured
   login-shell env from EMD-5, so the agent sees the same `PATH`,
   `HOME`, etc. the user gets in a terminal.
2. Overlay `provider_spec(provider).env` — per-provider overrides
   (e.g. `CLAUDE_CODE_*` vars when needed).
3. Overlay `agent_hooks::inject_hook_env_into(&mut env, port,
   token)` — the hook coordinates last, so neither the shell env
   nor a provider-specific override can clobber them.

### Start ordering

`AgentService` is constructed in `app.rs` setup **after**
`HookServer::start` resolves. The hook handle's `port()` and
`token()` are read synchronously into the service; once the service
is `app.manage`'d, any subsequent `agents.start` call already sees
the bound server. No flapping order between command registration
and the hook bind.

### Concurrency

`start` and `stop` both acquire `fs_lock.lock_for(task_id)` before
reading or writing `tasks.pty_id`. So a "Start agent" while
another "Stop agent" is in flight serializes through the same
per-task mutex EMD-17 uses for worktree mutation.

### `tasks.pty_id` column

`AgentService::start` writes the new PTY's id (as `text`) into
`tasks.pty_id`. `stop` clears it. Renderer reads the column via the
`tasks.list` command to render the "agent running" indicator.

### UiMutationEvents

Two new variants:

- `AgentStarted { task_id, provider }` — fires after a successful
  spawn + DB update.
- `AgentExited { task_id, exit_code }` — fires on stop. `exit_code`
  is `None` when the host kills the PTY before the child reports
  one; non-None when the renderer sees a clean exit.

### Commands

- `agents_start(task_id, provider, size, on_output)` → `PtyId`.
  Same `Channel<Vec<u8>>` pattern as `pty_spawn` so the renderer's
  xterm.js consumes the agent stream identically to a manual PTY.
  Wrapped in `tokio::task::spawn_blocking` because the PTY spawn
  call is synchronous and the Tauri command runtime needs the
  poll to return cheaply.
- `agents_stop(task_id)` — kills the PTY, clears `tasks.pty_id`,
  broadcasts `AgentExited`.
- `agents_list_providers()` → `Vec<ProviderSpec>` — for the
  renderer's picker.

### Hardened-runtime caveat

Some agent binaries need expanded entitlements (Bun's JSC needs
`allow-unsigned-executable-memory`; Node-based agents typically
don't). The packaging follow-up (EMD-22) audits each binary and
adds the narrowest entitlement set to the macOS bundle's
`entitlements.plist`. Documented here so the audit reviewer has the
checklist; no per-binary code branch lives in the agent-spawn
path.

## Consequences

### Easier

- "Click button, see agent" is one command call. Renderer can
  build the rest of the v1 demo on top.
- Provider matrix is a single match arm to extend.
- The dev-loop demonstration moment (terminal spawns an agent in
  a worktree) works without manual coordination.

### Harder

- We bake the hook coordinates into `AgentService` at construction
  time. A future rotation of the per-launch token would require
  re-constructing the service (or moving the coords into a
  `Mutex<Coords>`). v1 doesn't rotate, so the static capture is
  fine.
- The `_tasks_service_anchor` placeholder is intentional: a future
  refactor that surfaces typed task ops in the agent path can use
  `TasksService` directly; today we go through raw SQL because
  `tasks.pty_id` isn't on the renderer-facing `Task` struct.
- Hardened-runtime entitlements are not in this PR. The agent
  spawn path will work in unsigned dev builds for every provider;
  signed prod builds will need EMD-22 to land.
