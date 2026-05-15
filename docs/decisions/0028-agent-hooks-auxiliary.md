# 28. Agent-hooks auxiliary services

Date: 2026-05-15

## Status

Accepted (EMD-26).

## Context

ADRs 0021/0025/0026/0027 covered the core agent-hooks subsystem:
the hook server, the JSON-body classifier (claude), and the
text-stream classifier families A/B/C. The Electron build's
`core/agent-hooks/` directory has six more files that *aren't*
classifiers but are part of the same subsystem:

- `claude-trust-service.ts` — auto-trusts new worktrees in
  `~/.claude.json` so Claude Code's first-launch dialog doesn't
  block.
- `event-enricher.ts` — joins raw hook bodies with `task_id` /
  `project_id` by parsing the `pty_id` and looking up the
  `conversations` row.
- `notification.ts` — Electron-side OS notification dispatch on
  attention-worthy `AgentEvent`s.
- `hook-config.ts` — writes per-provider hook config files
  (`.claude/settings.local.json`, `.codex/config.toml`,
  `.pi/extensions/emdash-hook.ts`,
  `.opencode/plugins/emdash-notifications.js`) and the matching
  `.gitignore` entries.
- `agent-notify-command.ts` — renders the curl/PowerShell hook
  command strings that `hook-config` injects.
- `pi-emdash-extension.ts` + `opencode-notifications-plugin.js` —
  the actual script files that get materialized into the user's
  worktree.

## Decision

1. **`claude_trust` is its own top-level domain module** (not a
   submodule of `agent_hooks`). It serializes JSON config writes
   through a per-config-path `parking_lot::Mutex`, refuses to
   overwrite non-object roots or corrupt JSON, and writes
   atomically via temp+rename. The "auto-trust SSH" remote-FS
   path from the TS source is deferred until EMD-10 lands the
   SSH abstraction.

2. **`hook_config` is its own top-level domain module** with four
   internal submodules:

   - `assets` (`include_str!`) — embeds the pi extension and
     opencode plugin files verbatim from the Electron sources.
     Files are copied byte-for-byte under
     `src/hook_config/assets/`; bumping the TS file means
     copying it back here.
   - `notify_command` — renders the curl (Unix) / PowerShell
     (Windows) hook command argv and the Windows codex notify
     script body.
   - `path_probe::command_on_path` — cheap inline `which`. Adds
     no dep.
   - `gitignore::ensure_entries` — appends missing entries to
     the worktree's `.gitignore`, recognizing `dir/`, `dir/**`,
     and exact-match patterns as already-covered.

3. **Env-var names match the Electron contract**
   (`EMDASH_HOOK_PORT`, `EMDASH_HOOK_TOKEN`, `EMDASH_PTY_ID`).
   The original `agent_hooks::env::inject_hook_env_into`
   stubbed-out by EMD-9 used `EMDASH_AGENT_HOOK_*`; we rename
   here so the embedded plugin files (which can't be touched
   without diverging from the TS sources) keep working. EMD-27's
   spawn site already calls the function, and its branch will
   rebase onto this rename.

4. **`event_enricher` is NOT ported.** The Tauri hook server
   already populates `task_id` / `project_id` on `AgentEvent`
   via `agent_hooks::env::inject_hook_env_into` at spawn time
   (EMD-27 wires this), so the renderer-side join-on-read that
   the TS enricher performs is unnecessary in Tauri.

5. **`notification` is NOT ported as part of EMD-26.** OS
   notifications belong on the renderer side via
   `tauri-plugin-notification`; the trigger is the existing
   `UiMutationEvent::AgentHookEvent` → renderer-side
   `dispatch.ts` arm. Tracked as future renderer work, not
   re-host plumbing.

6. **`toml_edit` is the TOML library**, not `toml`. The codex
   notify writer merges into the user's existing
   `.codex/config.toml`; preserving comments and field ordering
   across the round-trip matters because users hand-edit that
   file. `toml_edit` does this natively where `toml` would lose
   them.

## Consequences

- The agent-hooks subsystem is now functionally complete in
  Tauri except for one renderer concern (OS notifications) and
  one cross-cutting concern (SSH remote auto-trust, blocked on
  EMD-10).
- Total surface added: ~720 LOC of Rust + 102 LOC of embedded
  TS/JS assets across 7 new files. 24 new unit tests (6
  claude_trust, 18 hook_config across submodules).
- Adds two crate deps: `regex` (already present transitively;
  promoted to direct in EMD-23) and `toml_edit` (new direct
  dep). `toml_edit` is widely used and maintained by the
  Rust-lang team.
- One coupled change: rename `EMDASH_AGENT_HOOK_*` →
  `EMDASH_HOOK_*` in `agent_hooks::env`. EMD-27's branch must
  rebase to pick this up; the rebase is mechanical because the
  only caller is the function signature itself.
- Wire format unchanged. No bindings regenerate needed.
