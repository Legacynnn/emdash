# 0022: GitHub provider v1 — octocrab + hand-rolled device flow

## Status

Accepted

## Context

EMD-13 ports the Electron build's GitHub integration. The TS stack
uses `@octokit/rest` + `@octokit/auth-oauth-device`; the Rust
equivalents are `octocrab` (de-facto GitHub crate) plus either
`oauth2` or a hand-rolled device-flow client.

Forces driving the decisions below:

- **One token store.** Every token (PAT, OAuth, gh-CLI shell-out)
  lands in the EMD-6 AEAD `app_secrets` layer under the same key.
  No plaintext on disk; no separate kv for "the GitHub token."
- **Two-step device flow with a renderer in the middle.** The
  Electron build polls inside one function; we don't get that
  luxury because the renderer needs the user code + verification
  URL before the user has authorized. `device_flow_start` and
  `device_flow_poll` must round-trip through the renderer.
- **Linux release builds reject OpenSSL.** Helmor pinned
  `rustls-tls` everywhere; we follow.

## Decision

### Client

`octocrab` 0.42 with features `default-client + rustls +
rustls-ring + rustls-webpki-tokio`. **No `native-tls`** anywhere in
the dep tree. `client_for_token(token)` is the single entry — every
GitHub-backed call constructs a fresh `Octocrab` from the stored
token.

### Auth — device flow

**Hand-rolled** via `reqwest` (not `oauth2`). The `oauth2` 4.x
typed builders insist on holding the `StandardDeviceAuthorizationResponse`
across the poll step; that's awkward when start + poll run in
different Tauri commands. Hand-rolling the two POSTs is
straightforward and matches what GitHub documents directly.

- `device_flow_start(scopes)` → `POST /login/device/code` →
  returns `{user_code, verification_uri, device_code, polling_interval_seconds, expires_in_seconds}`.
  Returned to the renderer so it can render the code + URL.
- `device_flow_poll(flow)` → `POST /login/oauth/access_token`
  with the device code, every `polling_interval_seconds` (slow_down
  responses bump the interval by 5 s). On success: persist token
  via `secrets.set("github.access_token", ...)`. On
  `expired_token` / `access_denied`: return `DenyOrExpired`. Hard
  cap at 10 minutes total.

The timing fields are `u32` (not `u64`) because specta forbids
BigInt types over IPC. GitHub's intervals (5 s) and expiries (15
min) fit comfortably.

### Auth — gh CLI fallback

`gh_cli_token()` shells out to `gh auth token`. Opt-in via the
renderer — we never read the gh keychain without an explicit user
click. On success, the token lands in the same AEAD slot via
`sign_in_with_token`.

### Identity persistence

A separate `app_settings` keyset (`github.identity.login` /
`.id` / `.name` / `.email` / `.avatar_url`). The token itself
never appears here. After a successful sign-in, the host calls
`fetch_viewer(client)` and writes the identity.

Identity is split from the token deliberately:

- The renderer needs the avatar + login on every mount; reading
  from `app_settings` is one cheap query vs an AEAD decryption.
- A token rotation (re-sign-in) shouldn't churn the identity —
  the renderer can render through it.

### OAuth scopes (locked)

`repo`, `read:user`, `user:email`, `read:org`. Documented in the
`OauthScope` enum so any future widening is a deliberate code
change.

### Token rotation

A 401 from any subsequent call maps to `GithubErrorCode::Unauthorized`.
The renderer's job is to redirect back to sign-in; the auth path
will overwrite the stale token. We do **not** auto-clear the
token on a single 401 (e.g. a flapping GitHub Enterprise instance
shouldn't sign the user out) — the next successful sign-in
overwrites.

### Rate limit handling

The Electron build reads `X-RateLimit-Remaining` / `Reset` headers
and surfaces them in a status bar. v1 of the Rust port maps
`octocrab::Error::GitHub { status_code: 403 }` to
`GithubErrorCode::RateLimited`; the host log records the `Reset`
epoch. A renderer "remaining-quota" widget is a v1.x follow-up
(the host emits the data; the renderer just reads it).

### GraphQL helper

**Deferred.** EMD-13 spec lists "PR reviewers / check runs /
comment threads" as the canonical GraphQL queries. We're not
shipping those queries in v1 — the renderer surface stops at the
PR list + diff. Once a renderer feature actually needs reviewers
or checks, we'll evaluate `octocrab::graphql` vs an in-house
helper at that point (not before).

### Renderer scope for v1

- Sign-in flow (device flow + gh CLI fallback)
- Identity card (login / avatar)
- Repo list
- PR list per repo

The **PR diff view** is acknowledged in the spec but lands as a
follow-up renderer pass. The diff command (`github_get_pull_diff`)
ships now so the host contract is fixed; the renderer feature
catches up in a UX-focused PR.

## Consequences

### Easier

- A second provider (Linear / GitLab / etc.) follows this same
  shape: AEAD token, identity in `app_settings`, two-step device
  flow if applicable, CLI fallback where the tool exists.
- The renderer never sees a plaintext token. The
  `unauthorized` / `rate_limited` codes are surfaced as typed
  envelopes; the renderer doesn't have to parse the GitHub error
  body.
- The dev-side `gh auth token` shortcut means a developer can
  exercise the full flow without OAuth round-tripping.

### Harder

- We commit to maintaining the hand-rolled device-flow path.
  Future GitHub schema changes need a corresponding update here.
  Mitigation: the start/poll endpoints are documented and stable;
  the change cost is small per release.
- `octocrab::Error` is large; we `#[allow(clippy::result_large_err)]`
  at the module level to keep `Result<T, GithubError>` ergonomic.
  An alternative would be boxing every variant, which propagates
  through callers and gains us nothing meaningful in practice.
- The PR-diff renderer UX is a follow-up. Until it lands, users
  see the PR list but can't view the diff in-app — they click
  through to GitHub.
