# 0006: PTY streaming primitive — load-test decision

## Status

Accepted. Confirms ADR-0003.

## Context

EMD-8 (PTY foundation) committed to `tauri::ipc::Channel<Vec<u8>>` as
the host→renderer streaming primitive, with sender-side coalescing at
16 KiB / 4 ms (ADR-0003). The choice was made on the strength of two
properties:

1. **JSON-array IPC overhead bypassed.** [tauri#13405] documents that
   `Vec<u8>` traffic through Tauri's regular IPC is JSON-encoded as a
   number array, with notable per-flush overhead. Channel payloads
   above ~1 KiB land on Tauri's raw-fetch transport instead. The 16
   KiB coalescer threshold puts every flush comfortably above that
   cutoff.
2. **Loss-tolerant by design.** `Channel::send` is fire-and-forget;
   the renderer reconciles on next `load()` if a chunk goes missing.
   Same loss-tolerance contract the renderer state-sync bridge
   inherits in EMD-7 / ADR-0004.

EMD-18 is the decision-gate that validates these claims with
measurements rather than vibes. The acceptance scenario: 10
concurrent PTYs piping `yes` for 60 seconds, with the same
coalescing parameters as production.

[tauri#13405]: https://github.com/tauri-apps/tauri/issues/13405

## Measurements

Harness: `cargo run --bin pty-loadtest -- --streams 10 --duration-secs 60`.
Run on an `aarch64-apple-darwin` (Apple Silicon) developer machine,
`master` of `2026-05-15`, debug build (no optimisations).

```
wall_time_s              = 60.001
streams                  = 10
command                  = "yes"

  stream id= 1  bytes=  648 MB  10.30 MiB/s  max_cb_us=21
  stream id= 3  bytes=  655 MB  10.42 MiB/s  max_cb_us=30
  stream id= 9  bytes=  655 MB  10.42 MiB/s  max_cb_us=39
  stream id= 6  bytes=  656 MB  10.43 MiB/s  max_cb_us=74
  stream id= 7  bytes=  646 MB  10.27 MiB/s  max_cb_us=219
  stream id=10  bytes=  652 MB  10.38 MiB/s  max_cb_us=442
  stream id= 4  bytes=  653 MB  10.38 MiB/s  max_cb_us=14
  stream id= 8  bytes=  671 MB  10.68 MiB/s  max_cb_us=21
  stream id= 2  bytes=  649 MB  10.33 MiB/s  max_cb_us=20
  stream id= 5  bytes=  666 MB  10.59 MiB/s  max_cb_us=34

total_bytes              = 6_555_020_878   (≈6.55 GiB)
aggregate_MiB_per_s      = 104.19
max_callback_us          = 442             (well below the 50_000 us / 50 ms threshold)
dropped_bytes            = 0
```

What the numbers mean:

- **Throughput**: ~10 MiB/s per stream, 100+ MiB/s aggregate. `yes` is
  the worst-case high-rate producer; real-world agent output is bursty
  at orders of magnitude lower bandwidth. The headroom is large.
- **Callback latency**: the receive-side callback is the host-process
  proxy for renderer frame time. A long callback would mean the
  webview's IPC thread is stuck on `Channel::send`; instead the worst
  case in the run was 442 µs — ~0.001 of a 16 ms frame. The renderer
  has full latitude to render at 60 fps without churn from the
  channel.
- **Dropped bytes**: zero on the host side. The coalescer is loss-less
  by construction; renderer drops would appear as a `Channel::send`
  failure that we intentionally don't surface (ADR-0003).

What the harness *can't* measure from outside the webview:

- Renderer-side JSON-decode time. We rely on the documented
  `Channel<T>` raw-fetch transport switch above 1 KiB to make this
  negligible. The host-side max-callback proxy is the best we can do
  without a real webview attached; the production renderer doesn't
  do JSON decode at all on this path.
- DOM repaint time (terminal cell rendering). xterm.js is fast enough
  that a 16 KiB chunk every 4 ms is well within its budget; not a
  Channel-primitive question.

## Decision

**Stay on `Channel<Vec<u8>>` with the EMD-8 / ADR-0003 coalescing
parameters.** Throughput and latency are both decisively sufficient
for the worst-case load the EMD-18 spec asks about; no fallback to
localhost WebSocket is warranted.

A localhost-WebSocket fallback would have meant:

- Per-session token authentication (an extra surface to design,
  document, and audit).
- Tauri capability bypass for one specific data path (the issue
  flags this as an explicit cost).
- Worse local-development ergonomics (the loopback socket would
  collide with whatever else the user has bound to 127.0.0.1).

None of those costs are justified by the numbers above.

## Consequences

### Easier

- We close the streaming-primitive decision permanently for the v1
  cycle. Any future regression on this path (e.g. a new Tauri release
  that drops the raw-fetch transport) gets caught by the same load
  test wired in CI.
- The PR ships the load-test harness as a permanent artifact. Anyone
  asking "should we move to a different primitive?" can re-run the
  numbers in 60 seconds.

### Harder

- We're betting on Tauri's raw-fetch transport staying above 1 KiB.
  Mitigation: the load test runs on every PR touching `src-tauri/`
  (non-blocking benchmark — emits a warning but doesn't fail), so a
  regression to the JSON-array path would show up as throughput
  collapse and trigger a follow-up.
- The harness measures the host-side, not the renderer-side. We
  accept this gap; the data is overwhelming enough that closing it
  empirically would be a confirmation, not a discovery.

## Follow-ups

- **Agent-output replay benchmark** (mentioned in the EMD-18 notes).
  Would replace the `yes` worst-case with recorded real-world agent
  traces. Filed as a follow-up issue rather than gating this PR.
- **CI matrix**: the load test runs on Linux / macOS-arm / macOS-intel
  / Windows as a non-blocking benchmark — same matrix as the rest of
  `tauri-dev-build.yml`. Failures emit warnings.
