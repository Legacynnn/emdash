# 0008: Auto-updater state machine + key custody + recovery plan

## Status

Accepted

## Context

EMD-15 scope is the **state machine, not the release pipeline.** Full CI
packaging + signing + rehearsal lands in EMD-22 because real signed
releases need real certs in CI. This ADR covers everything that *is*
in scope: the Helmor-style `UpdateManager`, retry backoff, progress
throttling, install-on-exit hook, the `latest.json` schema we'll
serve, the minisign key custody plan, and the per-platform recovery
procedure if an update bricks an install.

Three forces drove the specific shape:

- **One mutex around the state.** Tauri's plugin and our renderer both
  observe updater state. Without a single source of truth, "is a
  download in progress?" forks into two answers and the UI lies.
- **Backoff respects user intent.** A passive interval check that fails
  shouldn't keep hammering the server; a manual "Check for updates"
  shouldn't be silently delayed by a backoff timer that fired ten
  minutes ago.
- **Lose the minisign private key = lose the update channel forever.**
  No recovery path exists at the protocol level. Custody has to be
  documented and rehearsed before any signed release ships.

## Decision

### `UpdateManager`

A `parking_lot::Mutex`-protected `UpdateState` plus a `Backoff`
bookkeeper, plus a `Vec<EventListener>` for fan-out. The Tauri glue
registers one listener at startup that forwards into the renderer's
`Channel<UpdateEvent>`. State transitions are single-writer:
`begin_check`, `mark_up_to_date`, `mark_available`, `record_progress`,
`mark_ready_to_install`, `mark_failed` are the only ways state moves.

Stored in `tauri::State<Arc<UpdateManager>>` (managed in `app.rs`
setup) so the OnceLock-style "one instance per process" contract is
trivially upheld.

### Check reasons

`CheckReason = Startup | Resume | Focus | Interval | Manual`.

- **Manual** never schedules a backoff retry. The user is in the
  loop; they'll click again if they want.
- **Startup/Resume/Focus/Interval** retry on exponential backoff
  (below).

### Backoff

`BackoffConfig { initial: 30s, max_delay: 30min, multiplier: 2,
max_retries: 8 }`. The first six attempts hit 30s/1m/2m/4m/8m/16m;
attempts 7 and 8 clamp at 30m. After eight consecutive failures the
manager stops trying until the user opens the menu (a `Manual`
check resets the counter).

Numeric rationale: the upper bound matches the canary-release cadence
(roughly one release per 30 min during a bad day), so a user whose
network drops in and out once an hour catches each release within one
backoff cycle. `max_retries: 8` is the smallest number where
2^retries × initial exceeds a typical lunch break.

### Progress throttling

`PROGRESS_THROTTLE = 200ms`. `record_progress` updates the internal
state on every sample but only fires a `Downloading { progress }`
event if 200 ms has passed since the last emit *or* progress reaches
1.0 (forcing the completion edge through regardless). The renderer
can update its progress bar at 5 fps without re-rendering on every
chunk write — meaningful for slow networks where samples arrive in
bursts.

### Install on exit

`tauri::Builder::default().build(...).run(|_handle, event|
match event { RunEvent::Exit => { /* install hook */ } ... })`.

The hook is in place now; the call into the plugin's install path
lands with EMD-22 (the plugin path needs the signed bundle, which
needs the packaging pipeline). The state machine already knows when
to fire it: only if `UpdateState::ReadyToInstall { .. }`.

### `latest.json` schema

```
{
  "version":  "<semver of the published release>",
  "notes":    "<release notes; rendered as plain text>",
  "pub_date": "<ISO-8601 UTC, e.g. 2026-05-15T17:00:00Z>",
  "platforms": {
    "darwin-aarch64": { "signature": "<minisign>", "url": "<HTTPS>" },
    "darwin-x86_64":  { ... },
    "linux-x86_64":   { ... },
    "windows-x86_64": { ... }
  }
}
```

The `UpdateManifest` struct in `src/updater/manifest.rs` mirrors this
shape and is tested with serde round-trips. The endpoint URL lives in
`tauri.conf.json > plugins.updater.endpoints` with the standard
`{{target}}`, `{{arch}}`, `{{current_version}}` templates.

### Minisign key custody

**Dev keypair** — committed to this PR:

- Public key: embedded in `tauri.conf.json > plugins.updater.pubkey`
- Private key: **not committed**. Generated locally with `rsign generate`,
  used only for local "Check for updates" testing against a mock
  manifest. Loss of this key has zero production impact — a new dev
  pair gets generated.

**Production keypair** — generated separately, never committed:

- Generated on a machine that never touches CI (developer laptop,
  recorded in 1Password).
- Public key: replaces the dev pubkey in `tauri.conf.json` in the
  EMD-22 packaging PR. **The pubkey change is itself unsigned-update
  hostile**: an existing installation can't verify a manifest signed
  with a new key. We accept this — the dev pubkey is replaced before
  any signed release reaches users.
- Private key: stored in two places:
  1. **CI secrets** (GitHub Actions secret named
     `EMDASH_DEV_MINISIGN_PRIVATE_KEY`). Used by the packaging pipeline
     to sign release artifacts. Encrypted at rest in GitHub's secret
     store.
  2. **Offline backup**: 1Password vault entry **and** a hardware
     security key (Yubikey) held by the maintainer. Either alone is
     sufficient to recover; both together is the redundancy.
- Rotation: only on suspected compromise. Rotation breaks the update
  chain for everyone on a version stamped with the old pubkey —
  recovery is a manual reinstall (see below).

### Recovery plan

If an auto-update bricks an install (corrupt download, signature
verification failure on a published manifest, install hook crash
post-extract):

**macOS**
1. The user re-downloads the latest `.dmg` from the website. Manual
   install replaces the broken bundle.
2. App data (\`~/Library/Application Support/com.emdash.dev/\`) is
   preserved across reinstalls. The DB and AEAD master key live there.

**Linux (AppImage)**
1. The user re-downloads the latest AppImage. The old one is just a
   file; replace and re-run.
2. AppImage updates are the worst-served Tauri target — no official
   update channel beyond the manifest. We accept the user-visible
   "download and replace" flow; documented in the release notes for
   every published version.

**Windows (MSI)**
1. The user runs the latest MSI installer from the website.
2. Windows handles the upgrade as a normal MSI repair — preserves
   user data under `%APPDATA%\com.emdash.dev\`.

**Rollback procedure for a bad release**

If a release is found to be broken *after* the manifest is published:

1. Update `latest.json` on R2 to point back at the previous version's
   `darwin-aarch64`/`linux-x86_64`/etc. URLs and `pub_date`. Users
   running the bad release won't auto-downgrade (the updater only
   moves forward) but new auto-checks stop offering the broken bundle.
2. File a post-mortem issue with the failure mode + the rollback PR.
3. Announce in the release notes of the next good version so users on
   the bad one know to reinstall manually.

## Consequences

### Easier

- The state machine + backoff + throttling are all pure Rust logic
  with unit-test coverage (`src/updater/state.rs::tests`,
  `src/updater/backoff.rs::tests`). Nothing about the policy depends
  on a webview being attached, so regressions are catchable from CI
  without a UI harness.
- The dev "Check for updates" flow exercises every variant via
  `updater_simulate_event`. Adding a new `UpdateEvent` variant
  forces an update to the renderer's exhaustive `switch` and the
  domain-side `cfg(debug_assertions)` simulator.
- Recovery is documented before it's needed.

### Harder

- Two more PRs (EMD-22 for packaging + the eventual EMD-13 GitHub
  sign-in if we want signed-release tags) before the updater is
  end-to-end useful. The plumbing is in; the wiring is the long
  pole.
- The dev pubkey in `tauri.conf.json` is committed; it has to be
  swapped before the first signed release. We flag this loudly in
  EMD-22's checklist.
- Linux/AppImage update UX is bad and we own that we won't fix it in
  v1.
