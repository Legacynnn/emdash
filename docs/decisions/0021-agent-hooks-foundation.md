# 0021: Agent-hook server foundation + claude reference classifier

## Status

Accepted

## Context

Claude Code, Cursor, and most other agents emit hook events the host
needs to observe (notifications, tool-use signals, stop conditions).
The Electron build runs a local HTTP server agents POST to. EMD-9
ports the foundation: server + registry + one reference classifier
(claude). The remaining 28 classifiers split into EMD-23/24/25/26 so
the port parallelises.

Forces:

- **The hook events arrive from the user's machine, not from us.**
  An agent CLI we spawned is the publisher; the renderer is the
  consumer. We're a relay. The server has to be local-only.
- **Token validation has to be timing-safe.** A length-leaking
  string compare on the per-launch token would let a co-located
  process iterate one byte at a time.
- **Classifier behavior is the largest "mechanical but careful"
  port surface in the entire rewrite.** ~29 classifiers, each with
  its own shape. The foundation has to make adding the rest cheap.

## Decision

### Server

`axum` 0.7 (default-features off, `http1` + `json` + `tokio` only)
bound to `127.0.0.1:0`. The OS-assigned port lives on
`HookServerHandle::port()`; the per-launch token lives on
`.token()`. Both readable before any subsequent setup step.

One route: `POST /hook`. Token is validated with
`subtle::ConstantTimeEq` so the compare runtime doesn't depend on
where the bytes differ. Body must be JSON with at least a top-level
`agent: string` field; everything else is forwarded to the
classifier registered under that name.

### Classifier surface

```rust
pub trait Classifier: Send + Sync {
    fn name(&self) -> &'static str;
    fn classify(&self, body: &serde_json::Value) -> ClassificationResult;
}
```

`ClassificationResult { kind: AgentEventKind, message: Option<String> }`
where `AgentEventKind` is the typed enum the renderer reacts to:
`Notification { notification_kind }` / `Stop` / `Error` /
`Unknown`. Returning `Unknown` is fine — the server still
broadcasts so events aren't silently dropped.

Each classifier is a Rust module under `agent_hooks::classifier`
with insta snapshot tests against captured payload fixtures. This
PR ships the reference (`ClaudeClassifier`); EMD-23/24/25/26 land
the other 28 with the same shape.

### Registry

`ClassifierRegistry` is a `parking_lot::RwLock<HashMap<String, Arc<dyn Classifier>>>`.
Reads dominate writes (the registry is bootstrap-time-mutable in
practice), and the runtime registers all classifiers from `app.rs`
setup before the server is `start()`ed.

### Envelope

`AgentEvent` carries `agent` + `classifier` (so the renderer can
tell which side recognized the event), `kind`, optional `message`,
the enrichment `timestamp` (RFC 3339 string, because specta refuses
chrono types without a feature flag we'd rather not pin), and
`task_id` / `project_id` slots that stay `None` until **EMD-27**
(local agent invocation) wires them in.

Broadcast routes through `UiSyncManager::broadcast(UiMutationEvent::AgentHookEvent { task_id, event })`
to ride on the same Channel<UiMutationEvent> bridge as every other
cache-invalidation signal (ADR-0004).

### Env injection stub

`inject_hook_env_into(env, port, token)` is a free function that
populates `EMDASH_AGENT_HOOK_PORT`, `EMDASH_AGENT_HOOK_TOKEN`, and
the convenience `EMDASH_AGENT_HOOK_URL`. EMD-27 wires this into
the agent-spawn site. The stub exists today so the contract is
fixed before EMD-27 starts.

### Capability surface

No new Tauri capability. The hook server is host-internal; the
renderer reaches the events through the same Channel-based bridge
the rest of the app uses. CSP `connect-src` is NOT relaxed.

### Shutdown

`HookServerHandle::Drop` sends the `oneshot::Sender<()>` shutdown
signal and aborts the background task. Tauri's `AppHandle` holds
an `Arc<HookServerHandle>`; when the app exits, the Arc drops and
the server stops cleanly.

## Consequences

### Easier

- Adding a new classifier is a single Rust file + a test +
  `registry.register(...)` in `app.rs`. The shape is locked.
- The server is one bind, one route, one closure away from being
  tested in isolation — see `server::tests` for the three
  fixtures (no-token, wrong-token, valid-flow).
- The renderer cache-invalidation path is unchanged. The
  `AgentHookEvent` variant rides the existing `Channel<UiMutationEvent>`.

### Harder

- We commit to maintaining axum 0.7 in the binary. A future jump
  to 0.8 or beyond is a deliberate PR.
- Classifier behavior is *fixture-tested*, not exhaustively
  unit-tested. A new Claude Code hook event shape can land in
  prod before our classifier accepts it — the catch is the
  snapshot drift, not a runtime crash (Unknown falls through).
- The 28-classifier follow-up issues (EMD-23/24/25/26) inherit
  this PR's shape. If we want to change the shape later, the
  cost is touching all 29 modules.
