# Architecture Decision Records

This directory captures architectural decisions for the **emdash-dev** product
(the Tauri 2 + Rust rewrite under `src-tauri/`). It does **not** cover the
Electron emdash codebase, whose conventions live in `agents/architecture/` and
`agents/conventions/`.

## Format

Each ADR follows [Michael Nygard's template](https://github.com/joelparkerhenderson/architecture-decision-record/blob/main/locales/en/templates/decision-record-template-by-michael-nygard/index.md):

```
# {ADR-NNNN}: {short noun phrase}

## Status

{Proposed | Accepted | Deprecated | Superseded by ADR-XXXX}

## Context

What forces are at play? Constraints, prior art, open questions.

## Decision

What we will do.

## Consequences

What becomes easier and what becomes harder as a result.
```

## Conventions

- **Numbering**: four-digit zero-padded sequence. Never reuse a number, even
  if an ADR is superseded — supersede in place by updating Status.
- **Immutability**: once an ADR is Accepted, do not edit its Context or
  Decision. To change the decision, write a new ADR and mark the old one
  Superseded with a back-reference.
- **Scope**: ADRs are for decisions whose **rationale** would be hard to
  reconstruct from the code alone (e.g., a non-obvious version pin, a
  deliberate departure from upstream convention, a deferred refactor). Don't
  ADR things that are self-evident from a one-line code comment.
- **Authoring**: drafted in the same PR as the change they describe; reviewed
  alongside the code.

## Index

- [0000-template](./0000-template.md) — Template, not a real decision.
- [0001-initial-scaffold](./0001-initial-scaffold.md) — Tauri/specta version
  pins, crate layout, capability-allowlist mechanism, shell_env source.
- [0002-db-and-secrets-foundation](./0002-db-and-secrets-foundation.md) —
  Two-pool rusqlite topology, collapsed bootstrap migration, master key in
  OS keychain, per-row ChaCha20-Poly1305 AEAD.
- [0003-pty-streaming-via-channel](./0003-pty-streaming-via-channel.md) —
  PTY streaming on `Channel<Vec<u8>>` with sender-side coalescing.
- [0004-ui-mutation-event-bridge](./0004-ui-mutation-event-bridge.md) —
  Single `Channel<UiMutationEvent>` for renderer cache invalidation,
  enforced by `eslint-plugin-emdash`'s `no-tauri-event-bus` rule.
<<<<<<< HEAD
<<<<<<< HEAD
- [0020-file-watching](./0020-file-watching.md) — `notify` 6.x + 50 ms
  debounce, Linux inotify ENOSPC degrades to depth-1 watching with a
  user-facing toast (never polls). MCP orchestration deferred to a
  follow-up issue.
- [0005-git-worktrees-and-tasks-table](./0005-git-worktrees-and-tasks-table.md)
  — `git2` for read ops + shell out for `git worktree`,
  `WorkspaceFsMutationLock` for FS+DB serialization, JSON
  `source_branch` discriminator from day one.
- [0006-pty-streaming-loadtest-decision](./0006-pty-streaming-loadtest-decision.md)
  — Validates ADR-0003 with EMD-18 measurements: 10 streams × 60 s
  `yes` → 104 MiB/s aggregate, 442 µs max callback latency. Stay on
  `Channel<Vec<u8>>`; no localhost WebSocket fallback needed.
=======
- [0007-telemetry-privacy](./0007-telemetry-privacy.md) — Default-off
  telemetry pipeline; build-time host + key via `dotenvy`; user toggle
  in `app_settings`; bounded queue with oldest-drop on full;
  privacy contract for what's collected and where it goes.
>>>>>>> origin/danmelomour/emd-19-telemetry-port
=======
- [0008-auto-updater](./0008-auto-updater.md) —
  Helmor-style `UpdateManager` over `tauri-plugin-updater`, exponential
  backoff, 200 ms progress throttle, install-on-exit hook, minisign
  key custody plan, and per-platform recovery procedure.
>>>>>>> origin/danmelomour/emd-15-auto-updater-state-machine
