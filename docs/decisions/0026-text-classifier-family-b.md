# 26. Text-stream classifier family B (IDE-style)

Date: 2026-05-15

## Status

Accepted (EMD-24).

## Context

ADR-0025 ported family A (agent-style) on top of the
`BufferedTextClassifier` shape from ADR-0021. Family A worked
because all 7 agents shared the *same* ordered six-test set with
only the `idle_prompt` regex differing — so a single
`classify_common(text, idle_prompt_pattern)` helper covered them.

Family B (IDE-style: `cline`, `continue`, `copilot`, `droid`,
`gemini`, `junie`, `kilocode`, `kiro`) does not fit that mold.
Inspecting the TS sources:

- `cline` and `kilocode` emit a `Stop` event on task-completion
  markers (`Task completed`, `✓ Task Completed`).
- `continue` and `kilocode` use *multiple disjoint* idle-prompt
  regexes that don't OR together cleanly (one is anchored at end
  of line, the other isn't).
- `copilot` and `junie` carry bespoke permission patterns
  (`Do you want to|...`, `brave mode`).
- `droid` adds `API key valid` to its auth pattern.
- `gemini` is minimal — only `permission` and `idle`, with no
  auth / elicitation / error / stop detection at all.

Stretching `classify_common`'s one-parameter shape to cover all
of these would either (a) require adding several optional
parameters, turning the helper into a builder by stealth, or
(b) inline the regex tests in each agent file, duplicating
~30 lines per agent.

## Decision

1. **Introduce `ClassifySpec` + `run_spec` in `text_classifier`.**
   A declarative struct where every pattern is `Option`, the idle
   pattern accepts a slice (for the disjoint-pattern cases), and
   `stop` carries `(pattern, message)` so we don't lose the
   task-completion message:

   ```rust
   pub struct ClassifySpec {
       pub permission: Option<&'static str>,
       pub stop: Option<(&'static str, &'static str)>,
       pub idle: &'static [&'static str],
       pub auth: Option<&'static str>,
       pub elicitation: Option<&'static str>,
       pub error: Option<&'static str>,
   }
   pub fn run_spec(text: &str, spec: &ClassifySpec)
       -> Option<ClassificationResult>;
   ```

   `run_spec` walks the same ordered six-check set as
   `classify_common` (permission → stop → idle → auth →
   elicitation → error), skipping the slots that are `None`.
   Tail-vs-full anchoring matches the TS sources:
   tail-anchored: permission, stop, idle, elicitation;
   full-text-anchored: auth, error.

2. **Family B is 8 `const ClassifySpec` literals.** Each
   classifier is a 2-line `match_fn` that runs `run_spec(text, &SPEC)`
   plus a 3-line factory. Total ~150 LOC for the 8 specs, down from
   ~400 LOC across the TS sources.

3. **Family A's `classify_common` stays.** It's a strict subset of
   `run_spec` (idle-only, no stop, all other slots present), but
   refactoring family A would be churn for no behavior change and
   would touch a freshly-merged surface. Both helpers coexist;
   family C and beyond will use whichever fits.

4. **Default patterns are pulled into named constants** in
   `family_b.rs` (`PERM_DEFAULT`, `AUTH_DEFAULT`,
   `ELICIT_DEFAULT`, `ERROR_DEFAULT`) so spec literals stay
   readable and the inevitable "agent X uses the same permission
   pattern as agent Y" case is encoded once.

## Consequences

- Adding a new IDE-style classifier is now: define a const
  `ClassifySpec`, add a `match_fn` + factory, re-export from
  `family_b::mod`. ~10 lines per agent.
- No wire-format change. Family B emits the same
  `NotificationKind` variants that family A introduced; the
  `Stop` kind already existed in the foundation. `bindings.ts`
  and the wire-format snapshots are unaffected.
- The two-helper pattern (`classify_common` + `run_spec`) is
  intentional asymmetry, not duplicated work. Family A's
  factories are tiny one-arg calls. Family B's specs benefit
  from the optional slots. Forcing one shape to serve both
  would either bloat family A's call sites or weaken family B's
  type safety (every regex behind an `Option`).
- Family C (EMD-25) will likely use `run_spec` directly with
  more `None`s (kimi, letta, qwen are minimal-pattern like
  gemini). Family D, if it ever exists, can add another helper
  alongside.
