# Empty-frame test stops the stimulus before checking the watchdog

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The empty-frame test permits a watchdog reset on every frame.

## Details

F242 — **definite** — `e2e/tests/terminal-replay-rename.spec.ts:1276`;
`e2e/tests/terminal-replay-rename.spec.ts:1276–1278` — Empty-frame test stops the stimulus before checking the watchdog

The injector ends before the reveal deadline is checked. Even an incorrect watchdog rearmed by each empty frame reveals
the terminal shortly after traffic stops and satisfies the assertions. Require reveal while bounded empty-frame delivery
is still active, verify delivery after reveal independently, and stop the injector during failure cleanup.

## Evidence and triage context

- e2e/tests/terminal-replay-rename.spec.ts:1257 and :1271-1278 configure a 1,200 ms watchdog, inject real content, await
  ten empty frames 300 ms apart, and only then start a 15,000 ms reveal wait.
- e2e/tests/terminal-replay-rename.spec.ts:200-216 resolves injectEmptyFrames after the tenth delivery. The first
  delivery is immediate, so the nominal stimulus lasts 2,700 ms.
- crates/farhelm-ui/assets/terminal.js:4709 currently rejects empty frames. Without that return, :4723-4733 appends each
  empty chunk and rearms the idle timer while replay is buffered.
- crates/farhelm-ui/assets/terminal.js:4158-4163 replaces the existing watchdog; :4210-4211 ends catch-up with reason
  idle after the configured quiet interval. A final empty frame at approximately 2,700 ms therefore permits an idle
  reveal around 3,900 ms.
- crates/farhelm-ui/assets/terminal.js:4414-4417 records the idle reason; :4441-4459 joins buffered chunks into one
  write. Empty chunks add no content, so the single-write and retained-content assertions at
  e2e/tests/terminal-replay-rename.spec.ts:1279-1287 still pass.
- e2e/tests/helpers/terminal-readiness.ts:40-55 waits only for revealed to become true. It does not require reveal
  during injection or measure the original watchdog deadline.
- e2e/tests/terminal-replay-rename.spec.ts:203–213 resolves injection after the last frame.
- e2e/tests/terminal-replay-rename.spec.ts:1257 selects 1200 ms; :1276 awaits ten frames at 300 ms intervals; :1278 then
  allows another 15000 ms.
- e2e/tests/helpers/terminal-readiness.ts:40–55 starts a fresh poll and checks revealed, without checking its order
  relative to injection.
- crates/farhelm-ui/assets/terminal.js:4709 currently rejects empty frames; :4733 rearms the timer for accepted replay
  frames.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No runtime mutation experiment was performed.
- This does not establish an exploitable vulnerability in the frozen product: terminal.js:4709 contains the correct
  guard.
- The timing example assumes the browser is not stalled; that is sufficient to establish a permitted false pass.
- SPEC.md:1198-1200 and :1232-1238 describe replay and recovery. Neither those contracts nor SPEC_impl.md's
  terminal-flow rules accept this defective oracle.
- review_feedback_queue/birth-oracle.md:14-23 concerns filesystem capability-probe failures in checkout-ownership tests.
  Its trigger, consequence mechanism and scope do not cover this replay-watchdog test.
- The present product handler contains the correct empty-frame guard.
- No runtime mutation was performed.
- This is a distinguishing-oracle defect, not merely misleading commentary.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_13_cor:p1:F2`,
`test_infrastructure_13_sec:p1:F1`.

- `test_infrastructure_13_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
- `test_infrastructure_13_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
