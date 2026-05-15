# 25. Text-stream classifier family A (agent-style)

Date: 2026-05-15

## Status

Accepted (EMD-23).

## Context

ADR-0021 ("agent-hooks foundation") set up two parallel classifier
shapes:

1. **JSON-body classifiers** (`Classifier` trait) — used by the
   Claude Code hook server. Each hook event is a structured JSON
   POST, so the classifier matches on a typed field
   (`hook_event_name`).
2. **Text-stream classifiers** (`TextClassifier` trait, deferred to
   EMD-23/24/25) — used by every other agent. These have no hook
   server; we tail their PTY stdout and pattern-match on the
   rendered terminal text.

The Electron build groups text-stream classifiers into three
"families" based on how much they share:

- **Family A — agent-style** (this ADR). 7 classifiers
  (`amp`, `autohand`, `cursor`, `devin`, `jules`, `opencode`,
  `rovo`). Each one runs ANSI-stripped 4 KiB tail buffers and the
  *same* five-regex set; only the `idle_prompt` pattern differs per
  agent.
- **Family B — IDE-style** (EMD-24, deferred). cline, continue,
  gemini, kiro, ... — heavier formatting, sometimes multi-line.
- **Family C — niche** (EMD-25, deferred). kimi, letta, pi, qwen, ...
  — one-offs with bespoke patterns.

ADR-0021 chose `parking_lot::Mutex<String>` + a `feed(&self, &str)`
method shape to keep the trait `Send + Sync` from the PTY callback
thread; that decision applies here unchanged.

## Decision

1. **One shared matcher, parameterized per agent.**
   `classify_common(text, idle_prompt_pattern) -> Option<ClassificationResult>`
   in `agent_hooks::text_classifier` runs the full five-regex set
   (`permission_prompt`, `idle_prompt`, `auth_success`,
   `elicitation_dialog`, `error`). Each family-A agent is a
   one-line `MatchFn` that calls `classify_common` with its
   idle-prompt regex.

2. **Buffered sliding window in a single shared type.**
   `BufferedTextClassifier { name, matcher, buffer: Mutex<String> }`
   handles the 4 KiB tail buffer, UTF-8-safe trim, ANSI strip, and
   matcher dispatch. Per-agent factory functions just hand it a
   matcher: `cursor_classifier() -> BufferedTextClassifier`.

3. **Three new `NotificationKind` variants** —
   `IdlePrompt`, `AuthSuccess`, `ElicitationDialog` — added to
   `agent_hooks::event::NotificationKind`. These are emitted by the
   text-stream classifiers and serialize as snake_case so they're
   addressable from the renderer.

4. **ANSI strip is hand-rolled, not regex.** A single pass over the
   buffer recognizes the two cases we care about (CSI sequences
   `ESC [ ... letter` and OSC sequences `ESC ] ... BEL|ESC \`) plus
   carriage returns. This avoids pulling a second regex per call
   and keeps the strip pass cheap enough to run on every chunk.

5. **`idle_prompt` regexes are ported verbatim from TS sources.**
   They look like a 1990s grep — that's intentional. The Electron
   classifiers built up these patterns from real-world output over
   time, and changing them silently changes what notifications
   users see. Touching a pattern is a deliberate change, not a
   cleanup.

## Consequences

- Family A ports as ~80 lines of code total (the trait, the matcher
  helper, and 7 tiny factories), down from ~1500 lines split
  across `core/agent-hooks/classifiers/*.ts` + `base.ts` + the
  factory registry in the Electron build.
- Adding a new agent-style classifier is a 3-line change (one
  matcher fn + one factory fn + one re-export in
  `text_classifiers/mod.rs`).
- Family B and C will likely *not* fit `classify_common` as-is.
  Family B (cline, continue) needs multi-line/markdown-aware
  matchers; family C is a per-agent bespoke pattern set. Each
  family will add its own helper alongside `classify_common`
  rather than overloading it. (EMD-24/EMD-25 follow-ups.)
- We add a direct dependency on `regex` 1.x. It was already
  transitively present through `tracing-subscriber` / `notify`, so
  the binary size cost is zero.
- No wire-format change: this work is entirely internal to the
  classifier subsystem. The three new `NotificationKind` variants
  ride on the existing `AgentEvent` envelope, so no new `insta`
  snapshots are needed beyond the existing
  `tests/wire_format.rs` agent-hook coverage.
