# 0012: `editor/` subsystem (editor buffers) — port to v1 (S, lands in this PR)

## Status

Accepted

## Context

`src/main/core/editor/` (~70 lines) is a thin RPC controller over
`editorBufferService`: `saveBuffer`, `clearBuffer`, `listBuffers`,
all keyed by `(projectId, workspaceId, filePath)`. The DB table
already exists in the EMD-6 collapsed bootstrap (`editor_buffers`).

Used by the renderer's Monaco wiring to keep unsaved-buffer state
across restarts.

## Decision

**Port to v1 (S, lands in this PR).** The surface is tiny, the
schema is already in place, and we don't yet have a Monaco panel
to integrate with — but the host-side CRUD is cheap to ship now so
the renderer doesn't have to add a follow-up command set when the
editor UI lands.

**This PR ships:**
- `src/editor_buffers/` domain module (Tauri-runtime-free)
- Tauri glue: `editor_buffer_save`, `editor_buffer_clear`,
  `editor_buffer_list`
- specta types + capability allowlist entries + insta snapshots
- The EMD-7 template (domain → command → bindings → snapshot)

**Not in this PR:**
- Renderer Monaco panel (separate feature work)
- File-system reads of the canonical file contents (a `read_file`
  command will be added by the editor UI's PR; until then the
  buffers are pure DB state)

## Consequences

- Adds 3 commands and one small Rust module to the v1 surface.
- A renderer wishing to do unsaved-state preservation has the host
  contract ready to call.
- If the editor UI later needs different commands (e.g. partial
  buffer diff for crash recovery), they're additive.
