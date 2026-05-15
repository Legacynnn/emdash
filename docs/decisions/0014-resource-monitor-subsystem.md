# 0014: `resource-monitor/` subsystem — port to v1 (M)

## Status

Accepted

## Context

`src/main/core/resource-monitor/` (~165 lines) takes a one-shot
sample of CPU + memory for the running PTY processes and reports
them to the renderer's status bar. Gated by an `appSettings`
toggle so users can disable it on lower-end machines.

Pulls per-process stats — the Electron impl uses `ps -p` shell-out
on macOS/Linux and `Get-Process` on Windows. The Rust equivalent
is `sysinfo` (cross-platform), which adds ~150 KiB to the binary.

## Decision

**Port to v1 (M).** v1's user-visible value is "agent runs in a
worktree" — seeing CPU/mem per task is part of that loop. Without
the resource monitor, the user can't tell whether an agent is
busy, hung, or just slow.

**Scope (medium):**
- New domain module `src/resource_monitor/` using `sysinfo`
- Per-PTY sampling tied to the PTY ids from EMD-8
- `resource_monitor_snapshot(pty_id) -> Option<ResourceSample>`
  command + specta type + insta snapshot
- `app_settings.resource_monitor.enabled` toggle re-used (EMD-19's
  `app_settings` table covers this)

**Out of scope for v1:**
- Continuous streaming (a `Channel<ResourceSample>` would be the
  obvious next step). One-shot is sufficient until a feature needs
  more.
- Per-thread breakdown. Process-level only.

## Follow-up

File Linear issue **EMD-XX: Port resource-monitor subsystem (M)**.

## Consequences

- `sysinfo` becomes a dependency. Acceptable: maintained, MIT
  licensed, cross-platform, no native shims required.
- Resource numbers may differ slightly from the Electron build's
  numbers (different sampling implementations). Document the
  variance once the port is up.
