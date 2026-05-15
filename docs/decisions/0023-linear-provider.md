# 0023: Linear provider v1 — API-key auth + hand-rolled GraphQL

## Status

Accepted

## Context

EMD-14 ports the Electron build's Linear integration. The Electron
stack uses the official Linear SDK + GraphQL client. The Rust
equivalent in our footprint:

- **API-key paste only** for v1. OAuth is explicit v1.x; half-OAuth
  is worse than committed-to-one-mode.
- **Hand-rolled GraphQL** via `reqwest` (no `graphql-client` codegen).
  Linear's API is GraphQL-only and our v1 surface is ~14 queries —
  the codegen + schema-pin cost outweighs the boilerplate we'd save.
- **Same AEAD-only token storage** as EMD-13 (GitHub). Identity
  split into a plain `app_settings` keyset so the renderer doesn't
  decrypt on every mount.

## Decision

### Auth

`linear.sign_in(token)` takes the user-pasted API key, calls
`Secrets::set("linear.access_token", token)`, then fetches the viewer
to populate the identity row. `linear.sign_out` clears both.

**No OAuth in v1.** A future ADR documents the OAuth flow when the
v1.x port lands. The auth method choice is hidden from callers — the
authenticated `LinearClient` is constructed identically regardless
of how the token was acquired.

### Client

`LinearClient` wraps `reqwest` with `rustls-tls`. Single private
`query<T>(query, variables) -> T` method:

- POSTs to `https://api.linear.app/graphql` with the bearer token
- 15 s timeout
- Maps `401 / 403` to `LinearError::Unauthorized`
- Maps `429` to `LinearError::RateLimited(reset_epoch)` reading
  `X-RateLimit-Reset`
- Parses `data.<field>` from the GraphQL envelope; treats top-level
  `errors[]` as `LinearError::Graphql`

The Tauri command layer (`commands::linear`) wraps the client and
**clears the stored token on `Unauthorized`** — same revocation
discipline EMD-14 calls out. Renderer sees `LinearIdentityChanged`
and re-prompts.

### v1 entity surface (14 commands)

| Command | Mutation? | Purpose |
|---|---|---|
| `linear_sign_in(token)` | yes | Store token + populate identity |
| `linear_sign_out` | yes | Clear token + identity |
| `linear_me` | — | Read identity from `app_settings` |
| `linear_list_teams` | — | All teams in the workspace |
| `linear_list_projects(team_id)` | — | Projects under a team |
| `linear_list_cycles(team_id)` | — | Cycles under a team |
| `linear_list_labels(team_id)` | — | Labels under a team |
| `linear_list_states(team_id)` | — | Workflow states under a team |
| `linear_list_issues(filter)` | — | Issue list with team / project / cycle / state / assignee filter |
| `linear_get_issue(id)` | — | Single issue |
| `linear_create_issue(input)` | yes | Create + broadcast `LinearDataChanged{team}` |
| `linear_update_issue(id, input)` | yes | Update + broadcast `LinearDataChanged{team}` |
| `linear_list_comments(issue_id)` | — | Read comments on an issue |
| `linear_create_comment(issue_id, body)` | yes | Create a comment |

Out of scope (v1.x follow-ups): attachments, webhooks / real-time
subscriptions, sub-issue mutations (read-only in v1).

### Naming discipline

Every Linear-side type is prefixed `Linear` (`LinearIssue`,
`LinearTeam`, `LinearProject`, etc.) to avoid specta name
collisions with `projects::Project`, `tasks::Task`, and the
future GitHub/GitLab providers. The renderer-facing TS types thus
namespace cleanly without an explicit module-per-provider on the
TS side.

### UiMutationEvent

Two new variants:

- `LinearIdentityChanged` — sign-in, sign-out, refresh, or
  auto-revoke on 401.
- `LinearDataChanged { team }` — issue / comment writes invalidate
  the named team's view.

### Identity storage

Same pattern as GitHub: five `app_settings` keys
(`linear.identity.id` / `.name` / `.display_name` / `.email` /
`.avatar_url`). Token never appears here — only AEAD.

## Consequences

### Easier

- The Linear v1 surface is callable end-to-end from the renderer.
  Sign-in is one paste; every subsequent call is one command.
- Linear's GraphQL schema can evolve underneath us without the
  port breaking — the client decodes per-field, so additive
  changes are absorbed.
- A 401 auto-revokes the token, which is the safe default. The
  user sees the re-auth prompt; we don't keep a stale token alive
  through repeated failures.

### Harder

- 14 GraphQL queries are hand-written. A schema change that
  renames or removes one of our fields will surface as a
  `Malformed` error, not a typed mismatch. Mitigation: insta
  snapshots cover every command request shape; CI catches drift
  before a release ships.
- We're paying the cost of "no OAuth in v1" — the user pastes a
  raw API key from Linear's settings. Acceptable for the dev
  product audience.
- If a future feature wants attachments or webhooks, the deferral
  becomes a billable v1.x issue.
