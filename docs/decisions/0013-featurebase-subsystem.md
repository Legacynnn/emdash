# 0013: `featurebase/` subsystem — defer to v1.x

## Status

Accepted

## Context

`src/main/core/featurebase/` (~519 lines) integrates with the
FeatureBase product-feedback service: token storage, connection
health check, and an issue provider that lists FeatureBase tickets
in the app. Used to give users an in-app channel for feature
requests / bug reports.

## Decision

**Defer to v1.x.** Two reasons:

1. **Not on the critical path.** Users with feedback can email,
   Discord, or file a GitHub issue. An in-app FeatureBase panel is
   nice-to-have, not v1-blocking.
2. **Workspace integration is the bigger feature.** v1 should
   prioritize spinning up agents in worktrees and shipping the
   results; product-feedback channels can land in the v1.x sweep.

Follow-up Linear issue: **"emdash-dev FeatureBase integration"** —
file when the product team wants the in-app feedback channel back.

## Consequences

- Users have no in-app FeatureBase panel in v1.
- The Electron build's FeatureBase token storage (if any user has
  one) is **not migrated** — emdash-dev is fresh install only
  (ADR-0001 / ADR-0002), so no migration path exists anyway.
- If the FeatureBase API changes between now and the v1.x port, the
  v1.x issue will need to re-spec the integration.
