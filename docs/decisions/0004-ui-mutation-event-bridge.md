# 0004: Renderer state sync via `UiMutationEvent` bridge

## Status

Accepted

## Context

The renderer needs a single, typed pipeline for cache-invalidation
signals coming out of the Rust host. Without one, every feature ends up
inventing its own `app.emit("foo-updated", payload)` + renderer
`listen("foo-updated", ...)` pair, the contracts go untested, and the
"when do I refetch?" answer fragments across 20+ call sites. We already
lived this in Electron emdash.

Three forces pushed for resolution now (EMD-7 — the first real feature
PR on the Tauri stack):

- **One source of truth for cache invalidation.** Every domain mutation
  that should refresh the renderer must hit one well-known fan-out
  point. A reviewer should be able to grep one module to audit every
  invalidation rule.
- **Typed wire format, snapshot-pinned.** Tauri's IPC will silently
  reshape JSON if a backend struct gets a new field. Renderer breakage
  from that is invisible without a snapshot test.
- **Survivable enforcement.** Conventions decay. A CI-enforced ESLint
  rule keeps the discipline load-bearing without requiring reviewer
  vigilance every PR.

Reference architecture: Helmor's `UiSyncManager` plus its renderer
bridge hook, adapted to our domain-boundary discipline (the manager
must stay Tauri-runtime-free; see ADR-0001).

## Decision

We adopt a single `Channel<UiMutationEvent>` per renderer mount as the
*only* renderer state-sync subscription primitive. The spine has four
pieces:

1. **`UiMutationEvent` enum** — Rust-side, `#[serde(tag = "kind",
   rename_all = "snake_case")]`, exported via `tauri-specta` to the
   renderer. Initial variants:
   `ProjectCreated { id } | ProjectUpdated { id } | ProjectDeleted { id }`.
   New variants are added per feature PR.

2. **`UiSyncManager`** — domain-only struct (`src/ui_sync/manager.rs`)
   that owns a `HashMap<String, SubscriberCallback>` behind a
   `parking_lot::RwLock`. Exposes `subscribe(sub_id, callback)`,
   `unsubscribe(sub_id)`, `broadcast(event)`. The manager **never sees
   `tauri::ipc::Channel<T>`**. The Tauri glue in
   `src/commands/ui_sync.rs` wraps a `Channel<UiMutationEvent>` into a
   `SubscriberCallback` at registration time. This keeps
   `UiSyncManager` link-clean from the `emdash-cli` binary that
   enforces the domain/glue split.

3. **Tauri commands**:
   - `subscribe_ui_mutations(sub_id: String, on_event: Channel<UiMutationEvent>)`
   - `unsubscribe_ui_mutations(sub_id: String)`
   The renderer generates a fresh UUID v4 per `App` mount and
   unsubscribes explicitly on teardown.

4. **Renderer bridge**:
   - `ui-sync/useUiMutations.ts` — root-mount React hook; opens one
     channel, hands every incoming event to `dispatchUiMutation`.
   - `ui-sync/dispatch.ts` — a single `switch` over `event.kind`
     mapping each variant to a MobX store mutation. This is the only
     module that translates wire events into store side-effects.

Every domain write that should invalidate renderer state calls
`UiSyncManager::broadcast(event)`. Domain code never touches Tauri
types; only `commands/projects.rs` (and equivalents) translates
`ProjectsService::add(...) -> Project` into a follow-up
`manager.broadcast(UiMutationEvent::ProjectCreated { id: ... })`.

### Discipline enforcement

A local ESLint plugin (`src-tauri/ui/eslint-plugin-emdash`) ships a
`no-tauri-event-bus` rule, wired at `"error"` in `eslint.config.js`.
The rule reports:

- `listen` / `once` imports from `@tauri-apps/api/event` anywhere
  outside `ui-sync/`.
- Bare calls to those bindings.
- `*.emit(...)`, `*.emit_all(...)`, `*.emit_to(...)` member calls
  (heuristic for host-side broadcasts in TS files; the Rust side has
  no `tauri::Emitter` import path that survives the domain-boundary
  test).

`pnpm lint` (run in CI) fails on a single violation. A
`// emdash-disable-next-line no-tauri-event-bus -- <reason>` pragma
exempts genuine exceptions; one exemption already exists in
`useUiMutations.ts` covering the `Channel<UiMutationEvent>`
construction itself.

### Wire-format pinning

`tests/wire_format.rs` carries one `insta::assert_json_snapshot!` per
`UiMutationEvent` variant plus request-arg snapshots for the four
commands (`subscribe_ui_mutations`, `unsubscribe_ui_mutations`,
`projects_list`, `projects_add`, `projects_remove`). Drift requires
`cargo insta accept` — same friction as ADR-0003's PTY snapshots.

### Loss tolerance

`Channel::send` is fire-and-forget. We accept the same loss-tolerance
contract as the PTY data channel (ADR-0003): a renderer that misses a
mutation event reconciles on next `load()` of the affected store. The
list-style stores re-fetch from authoritative state; optimistic
mutations are kept out of v1.

## Consequences

### Easier

- Adding a new feature is a fixed template: domain module + Tauri
  command + specta type + capability entry + insta snapshot +
  `UiMutationEvent` variant + dispatch arm + store action. EMD-7 ships
  the reference (`projects` end-to-end); every subsequent feature
  issue should look like this PR.
- Cache-invalidation auditing collapses to two files:
  `commands/<feature>.rs` (where broadcasts originate) and
  `ui-sync/dispatch.ts` (where stores react). No archaeology required.
- The exhaustiveness check on the dispatcher's `switch` turns a
  missed variant into a compile error.

### Harder

- Channel<T> overhead per mutation event. Acceptable while we treat
  events as low-bandwidth invalidation signals; high-bandwidth data
  (PTY) keeps its own `Channel<Vec<u8>>` per session per ADR-0003.
- The ESLint rule is an extra surface to maintain; a future plugin
  upgrade can break us. Mitigation: the plugin is local (in-repo) and
  vendor-free (typescript-eslint + emdash plugin only).
- Wire-format reshapes (renaming an enum variant, adding a field) now
  require a snapshot review step. This is the goal, not a cost.
