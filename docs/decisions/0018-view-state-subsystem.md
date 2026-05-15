# 0018: `view-state/` subsystem — port to v1 (S, lands in this PR)

## Status

Accepted

## Context

`src/main/core/view-state/` (~39 lines) is a key/value store
keyed on a renderer-supplied string, value is opaque JSON. Used to
persist things like "which tab was open in the right panel," "what
size are the resizable splits," etc. — UI state that's nice to
keep across launches but not worth a typed setting.

Backed by the `kv` table from the EMD-6 collapsed bootstrap.

## Decision

**Port to v1 (S, lands in this PR).** Tiny surface; the table
exists; we can ship it now so the renderer has the contract ready
when feature work starts persisting UI layouts.

**This PR ships:**
- `src/view_state/` domain module (Tauri-runtime-free)
- Tauri glue: `view_state_save`, `view_state_get`,
  `view_state_get_all`, `view_state_delete`, `view_state_reset`
- specta types (`unknown` → `serde_json::Value`); capability
  allowlist entries; insta snapshots

The keys live in the same `kv` table the EMD-6 bootstrap defines; no
schema change.

## Consequences

- 5 commands added to the v1 surface — small.
- The renderer can persist any UI state by JSON-stringify-ing it
  into the typed store, without needing to coordinate with the typed
  settings layer.
- If a key needs schema enforcement, port it to the typed settings
  layer (ADR-0017) instead.
