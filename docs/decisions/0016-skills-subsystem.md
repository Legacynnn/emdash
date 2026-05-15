# 0016: `skills/` subsystem — defer to v1.x

## Status

Accepted

## Context

`src/main/core/skills/` (~704 lines) manages Claude/Codex "skills"
— named bundles of prompts + workflows that agents can invoke
mid-conversation. Surfaces: catalog index, refresh from a bundled
JSON catalog, install/uninstall individual skills, list installed,
plus the file system layout under `~/.emdash/skills/`.

Built on top of an evolving Claude skills format; the spec is
still moving (skill manifests, dependency resolution, lockfiles).

## Decision

**Defer to v1.x.** Two reasons:

1. **Upstream spec churn.** Locking emdash-dev v1 to a specific
   skills schema risks shipping with stale assumptions. Better to
   re-port once the upstream stabilizes.
2. **Not on the v1 critical path.** v1's user story is "spawn an
   agent in a worktree, get work done." Skills are a power-user
   enhancement that's easy to add additively in v1.x.

Follow-up Linear issue: **"emdash-dev skills subsystem port"** —
file when the upstream skills spec stabilizes (or when a user-
visible feature in v1.x specifically requires it).

## Consequences

- Users who installed skills in the Electron build don't see them
  in emdash-dev. Acceptable: fresh-install product (ADR-0001), no
  migration path anywhere.
- The renderer's "Skills" tab disappears from v1. The agent
  invocation path (EMD-27) doesn't need it — agents call skills
  through their own runtime, not through us.
