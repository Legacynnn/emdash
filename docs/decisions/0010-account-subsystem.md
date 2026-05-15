# 0010: `account/` subsystem — defer to v1.x

## Status

Accepted

## Context

`src/main/core/account/` (~124 lines) is the Electron-side wrapper
for emdash's own backend account service. Surfaces: `getSession`,
`signIn(provider?)`, `signOut`, `checkHealth`, `validateSession`,
plus the `provider-token-registry` that brokers third-party tokens.

The service talks to a hosted emdash backend (`emdashAccountService`)
that doesn't exist for emdash-dev yet. The new product is fresh
install only; we haven't decided whether emdash-dev will share that
backend or stand up its own.

## Decision

**Defer to v1.x.** Don't port `account/` in v1. Three reasons:

1. No v1 user-visible feature requires it. GitHub sign-in (EMD-13)
   gives us "who am I" for the telemetry `user.identify` event
   (EMD-19 / ADR-0007).
2. The hosted backend's API surface isn't stable enough to commit to.
3. Provider-token brokerage (the only other piece in this module) is
   already covered by `app_secrets` via EMD-6 — each provider port
   (GitHub, Linear) talks to that layer directly.

Follow-up Linear issue: **"emdash-dev account service backend
bridge"** — opens when v1.x acquires a concrete need (paid features,
cross-device settings sync, etc.).

## Consequences

- Users with an emdash.sh account see no integration in emdash-dev.
  Acceptable because the new product hasn't been marketed yet.
- Telemetry `user.identify` works without this module, sourcing
  identity from GitHub instead.
