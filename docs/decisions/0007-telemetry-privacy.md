# 0007: Telemetry pipeline + privacy contract

## Status

Accepted

## Context

EMD-19 ports `src/main/lib/telemetry.ts` from Electron emdash to Rust.
The umbrella issue locks the v1 policy: **ship the pipeline off by
default until the new collector is validated.** This ADR records what
the pipeline collects, where it sends, and how the user opts in or
opts out.

The Electron implementation runs in production with telemetry on and
a clear opt-out toggle. We're not in a position to repeat that yet:

- The receiving endpoint may be different from Electron emdash's; the
  decision to repoint vs reuse is out of scope here.
- We haven't validated event-shape parity against a real collector
  (Mixpanel / PostHog / a custom ingest — TBD).
- A canary release with telemetry enabled can come in a follow-up
  once both of the above are settled.

So v1 ships the *code path*. Flipping the default on is a separate
decision the user (Daniel) will make in a future ADR.

## Decision

### Compile-time configuration

`build.rs` reads `EMDASH_TELEMETRY_HOST` and `EMDASH_TELEMETRY_API_KEY`
via `dotenvy` (looking at `src-tauri/.env`, which is gitignored) and
emits `cargo:rustc-env=...` so the runtime can read them via `env!`.

An empty `EMDASH_TELEMETRY_HOST` at compile time disables the pipeline
*entirely* — `TelemetryConfig::is_compiled_in()` returns `false` and
`Telemetry::record` becomes a no-op. This means:

- Local dev builds without a `.env` file: telemetry is dead-code from
  the binary's perspective.
- CI builds: same — no `.env`, no host, no traffic.
- Release builds: the host + key are injected via CI secrets, and the
  pipeline becomes *capable* of sending. Whether it actually does
  depends on the user toggle.

### Runtime gates

Three checks gate every send:

1. **Compile-time host stamped in.** Covered above.
2. **User toggle in `app_settings`.** Key `telemetry.enabled`,
   defaults to `false`. The renderer's "Send telemetry to help
   improve emdash - dev" checkbox writes this.
3. **Queue capacity.** Bounded `tokio::sync::mpsc::Sender` of size
   256. A full queue means the worker is behind — `try_send` fails
   and the producer drops the event immediately. **Drop policy:
   oldest.** The newest event in a stalled flow is the most likely
   to be the one the user cares about (a freshly-triggered action,
   not stale focus-fluttering); we'd rather lose what's already in
   the queue.

### What gets sent

Initial event set, mirroring Electron:

- `app.focus` — fires on `window.focus` in the renderer (no Tauri
  event-bus subscription — see ADR-0004 / `no-tauri-event-bus`).
- `app.unfocus` — fires on `window.blur`.
- `app.dau_ping` — fires once per app-root mount. Sufficient to
  count distinct active days per `instance_id`.
- `user.identify` — fires after a successful GitHub sign-in
  (EMD-13). Carries `gh_username`, `gh_account_id`, and `email` as
  optional properties. **Not wired yet** (EMD-13 is in backlog);
  the command shape is present so a follow-up doesn't need to add
  it.

Every envelope carries the same base properties:

- `platform` (`std::env::consts::OS`)
- `arch` (`std::env::consts::ARCH`)
- `app_version` (`CARGO_PKG_VERSION`)
- `schema_version` (currently `1`)

Plus per-event properties. The receiver is expected to ignore
unknown properties — schema additions don't break old payloads.

### Identity

A `telemetry.instance_id` (UUID v4) lands in `app_settings` on first
enabled send and is reused across restarts. It carries no PII and is
not derivable from anything user-visible.

`user.identify` is the *only* event that carries human-identifying
data (the optional GitHub fields), and it only fires after the user
signs in. Anyone who never signs in produces zero PII.

### Transport

- `reqwest` with `rustls-tls` (no OpenSSL dependency on Linux).
- HTTPS POST to `<host>` with a `Bearer <api_key>` header.
- 5 s per-request timeout.
- One background `tokio::spawn` worker; the producer side never
  awaits the network.

### Shutdown

`Telemetry::shutdown` drops the producer-side `Sender` so the worker
observes channel close, then waits up to **200 ms** for in-flight
events to drain. Anything not POSTed by then is dropped. The user
closed the app; we don't make them wait.

## Privacy contract

| Question | Answer |
|---|---|
| What's collected? | Anonymous usage signals: focus/unfocus, DAU pings, and (post sign-in) the user's GitHub profile fields. No file contents, project paths, agent prompts, or terminal output. |
| Where does it go? | The host stamped in at build time. **TBD for v1** — defaults empty, so nothing leaves any local-built binary. |
| What's stored locally? | `app_settings.telemetry.enabled` (the toggle, default `false`) and `app_settings.telemetry.instance_id` (UUID v4 — sticky per installation). |
| How long is it retained? | At the receiver's policy. We commit to the receiver having documented retention before we flip the default on. |
| How does the user opt out? | Settings → "Send telemetry to help improve emdash - dev". Off by default. Flipping it back off stops new events immediately. |
| Can the user delete their data? | A "Forget me" affordance is a follow-up. Until then, the user can rotate their `instance_id` by deleting `app_settings.telemetry.instance_id` (no UI; manual SQL). |

## Consequences

### Easier

- One pipeline, one toggle, one place to audit what flows to a third
  party. Future events follow the same `TelemetryEvent` enum and
  inherit every gate by construction.
- The compile-time gate is load-bearing: a dev build cannot accidentally
  ship traffic because `.env` is gitignored and CI doesn't inject the
  vars on unsigned dev artifacts.
- Default-off keeps us honest until the receiver is settled.

### Harder

- "Default off" almost certainly means a meaningful undercount versus
  Electron emdash, which ships default-on. We accept the gap — the
  cost of misconfiguring a still-validating pipeline outweighs the
  visibility cost.
- A future "flip default on" decision will need its own ADR (privacy
  notice update, settings UX review, canary release plan).
- We can't easily detect compromised payloads on the receiver from
  the client. Mitigation: the `Bearer` header is the only auth; rotate
  the API key on any incident.
