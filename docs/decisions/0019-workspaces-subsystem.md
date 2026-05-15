# 0019: `workspaces/` subsystem — port the core to v1, defer BYOI

## Status

Accepted

## Context

`src/main/core/workspaces/` (~1741 lines) is the workspace
abstraction that sits between a task and its on-disk environment.
Three concrete kinds:

- **Local worktree** — git worktree under the project root. Covered
  by EMD-17 (tasks + worktrees).
- **SSH** — remote shell connection. Covered by EMD-10 (SSH + remote
  PTY).
- **BYOI** (Bring-Your-Own-Infra) — `byoi/` subdirectory. Users can
  point a task at a custom workspace they own — a sandbox container,
  a VM, a third-party dev environment. Heavy: requires sandbox
  provisioning protocols, status reporting, lifecycle management.

The `WorkspaceBootstrapService` + factory pattern handles "given a
project + task, resolve to a usable workspace handle."

## Decision

- **Port the core workspace abstraction to v1 (M).** Already
  in-progress across EMD-17 (local) and EMD-10 (SSH); this ADR
  records the shape.
- **Defer BYOI to v1.x.** Material additional complexity (sandbox
  protocols, third-party API contracts) that doesn't show up on the
  v1 critical path.

**v1 ships (across EMD-17 + EMD-10):**
- `Workspace { kind: Worktree | Ssh, path, ... }` model
- Per-task workspace creation/destruction (worktree add/remove for
  local; SSH session attach/detach for remote)
- `WorkspaceFsMutationLock` (already in EMD-17) for serializing
  FS+DB updates per workspace
- The `workspaces` table from the EMD-6 collapsed bootstrap

**v1.x picks up:**
- BYOI provider protocol
- Custom workspace state-reporting commands
- Third-party VM / sandbox lifecycle hooks

## Follow-ups

- **None at the v1 level** — EMD-17 and EMD-10 cover the core.
- **EMD-XX: BYOI workspace provider (v1.x)** — file when a partner
  asks for it, or when the v1 product stabilizes and we need a
  differentiator.

## Consequences

- The Electron build's BYOI users see no integration in emdash-dev.
  Acceptable — BYOI is power-user, not the v1 audience.
- The workspaces table is populated with `kind in ('worktree',
  'ssh')` only for v1. Adding BYOI later is a schema-compatible row
  addition.
