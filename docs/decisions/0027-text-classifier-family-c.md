# 27. Text-stream classifier family C (niche)

Date: 2026-05-15

## Status

Accepted (EMD-25).

## Context

ADR-0026 set up `ClassifySpec` + `run_spec` as a flexible
declarative shape that handles family B's heterogeneity. Family C
(`auggie`, `charm`, `codebuff`, `freebuff`, `generic`, `goose`,
`kimi`, `letta`, `mistral`, `pi`, `qwen`) is 11 small one-offs.
Most fit `ClassifySpec` directly. Two cases force exceptions.

## Decision

1. **Reuse `ClassifySpec` from ADR-0026 wherever possible.** Nine
   of the 11 niche classifiers are const `ClassifySpec` literals
   plus a 2-line matcher. This brings the family-C port to
   ~150 LOC for 9 classifiers, in line with families A and B.

2. **Mistral gets an inline `match_fn` because of a
   tail-length-gated idle marker.** Its last-resort idle pattern
   (`\bvibe\s*>|›|»|>`) only fires when the tail is shorter than
   100 chars, to avoid over-firing on any output ending with `>`.
   That predicate can't be expressed in a const spec, so the
   matcher manually runs `run_spec(pre)` → length-gated check →
   `run_spec(post)` in the original order. The pre/post specs
   each cover half of mistral's check set, keeping the regex set
   itself declarative.

3. **Pi's `stop` check is hoisted to its spec position (before
   `idle`).** In the TS source pi's `stop` (matching the JSON
   `"type":"agent_end"` marker) is the second-to-last check, not
   the second. Since the stop pattern is JSON-shaped and can't
   coincide with an idle marker in real PTY output, the natural
   spec position is behaviorally equivalent. Documented inline in
   the spec.

4. **Verbatim regex porting, even when the TS source contains
   apparent bugs.** `kimi.ts` includes a `qwen\s*>` pattern in
   its idle list — almost certainly a copy-paste artifact, but
   silently changing user-facing classifier behavior risks
   regressions we can't easily detect (the idle-prompt firing
   rate is what drives toast frequency). The pattern is ported
   intact with an inline comment noting the suspicion.

## Consequences

- The agent-hooks subsystem now classifies output from 28
  agents (26 text-stream across families A/B/C + claude via
  JSON-body classifier + the special-case "freebuff" codebuff
  variant). EMD-26 (claude-trust-service, event-enricher,
  notification, hook-config, pi-emdash-extension) is the last
  remaining slice of agent-hooks scope.
- `ClassifySpec`'s flexibility is validated by family C's mix
  of variations: bespoke permission patterns, optional stop
  patterns, varying numbers of idle patterns, and extended
  auth-success vocabularies. Future agents almost certainly fit
  the spec; the mistral pattern (custom predicate) is the
  escape hatch.
- No wire-format change. Family C reuses the `NotificationKind`
  variants introduced in EMD-23.
