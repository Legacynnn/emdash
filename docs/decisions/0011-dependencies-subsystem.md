# 0011: `dependencies/` subsystem — port to v1 (M)

## Status

Accepted

## Context

`src/main/core/dependencies/` (~848 lines) detects required CLIs on
the user's PATH (`git`, `gh`, `codex`, `claude`, `cursor`, etc.),
reports their versions, and can run install commands for the ones
that ship with installers. Heavily used by the renderer's
"Dependencies" panel and gates a few flows (no `git` = no
worktrees; no agent CLI = no agent-spawn).

## Decision

**Port to v1 (M).** v1 needs this — most of the agent / worktree
flows assume the relevant CLI is present and surface useful errors
when it isn't. Without `dependencies`, the renderer can't show
"install codex" before the user tries to spawn an agent and gets a
confusing `command not found` from the PTY.

**Scope (medium):**
- `Dependency` model + registry
- Cross-platform probe: `which`/PATH scan + version parsing (parse
  `--version` output)
- Install runner: per-OS package-manager shell-out (`brew install`,
  `winget install`, etc.) — kept minimal; documented limits
- IPC commands matching the Electron surface (`getAll`, `get`,
  `probe`, `probeAll`, `probeCategory`, `install`)
- `UiMutationEvent::DependencyChanged { id }` for cache invalidation

**Out of scope for v1:**
- SSH-side dependency probing (the `connectionId` parameter exists
  in Electron for that). Deferred to v1.x; lands with EMD-10's SSH
  follow-ups.
- Auto-install on app start. Always user-initiated.

## Follow-up

File Linear issue **EMD-XX: Port dependencies subsystem (M)**. Won't
land in this PR; the ADR records the decision and the boundary so
EMD-27's local-agent-invocation work doesn't accidentally re-invent
the probe layer.

## Consequences

- v1 has a Dependencies panel that the user can run before spawning
  agents.
- The renderer's "install Codex" CTA is one click. Without this
  subsystem, that becomes "open a terminal and `brew install
  codex`" — workable but worse.
