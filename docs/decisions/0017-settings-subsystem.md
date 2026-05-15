# 0017: `settings/` subsystem — port to v1 (M)

## Status

Accepted

## Context

`src/main/core/settings/` (~593 lines) is the typed app-settings
layer: a JSON-Schema-validated key/value store with per-key defaults,
override merging, telemetry on update, and command surfaces for
`get`, `getAll`, `getWithMeta`, `update`, `reset`, `resetField`.
Used by the renderer for every persisted preference (theme,
resource-monitor toggle, telemetry toggle from EMD-19, etc.).

EMD-19 already touches one bit of the surface — the `telemetry.enabled`
toggle persists in `app_settings`. EMD-21's "port settings" is the
fuller surface around it.

## Decision

**Port to v1 (M).** Required:

1. The renderer needs typed reads/writes for the dozen or so
   settings that exist (theme, fonts, resource-monitor cadence,
   shortcuts, etc.).
2. EMD-19's `telemetry.enabled` is currently bespoke; consolidating
   under the typed settings layer keeps invariants in one place.

**Scope (medium):**
- `src/settings/` domain module
- `AppSettings` struct + serde defaults (covers every Electron
  setting one-for-one; verify against `src/main/core/settings/schema.ts`)
- `set / get / update / reset / get_with_meta` commands with
  specta + capability + insta snapshots
- `UiMutationEvent::SettingChanged { key }` for cache invalidation
- Migrate EMD-19's `telemetry.enabled` toggle into the typed layer
  (one-line redirect)

**Out of scope for v1:**
- Per-project overrides (Electron's `project_settings` table). The
  schema is in the EMD-6 bootstrap; the API can land in a v1.x port.
- Settings sync across devices (depends on `account/`, which is
  deferred per ADR-0010).

## Follow-up

File Linear issue **EMD-XX: Port settings subsystem (M)**.

## Consequences

- The renderer has a single, typed settings API for every preference.
- Telemetry's bespoke toggle gets folded in once the M-port lands.
- A v1.x port to add per-project overrides is straight-forward since
  the schema and the typed layer are in place.
