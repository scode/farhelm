# Work-order progress reads can observe an empty file during truncation

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An empty progress baseline could make old output look newly produced.

## Details

F314 — **possible** — `e2e/tests/work-start-order.spec.ts:132; e2e/tests/work-start-order.spec.ts:170` — Work-order
progress reads can observe an empty file during truncation

The producer prints output before truncating and rewriting its progress file. A baseline read in the truncation gap
becomes zero through numeric conversion; completion of that already-started write can then satisfy the required
increment without the claimed subsequent emissions. Both initial and mixed-state baselines are affected. Reject empty or
malformed baselines or publish atomically. The interleaving was not reproduced, and other ordering checks do not
establish this producer witness.

## Evidence and triage context

- e2e/tests/work-start-order.spec.ts:68 prints terminal output before truncating and rewriting the progress file. A read
  between truncation and the write can return an empty string.
- e2e/tests/work-start-order.spec.ts:132-141 converts baseline reads directly with Number. If a baseline is read empty
  while the actual counter exceeds 30, completion of that already-started write can satisfy the
  greater-than-baseline-plus-30 check without 30 subsequent emissions.
- e2e/tests/work-start-order.spec.ts:168-174 claims output must advance past the mixed-state observation, but an empty
  mixedProgress baseline followed by publication of a count whose terminal output preceded that observation can satisfy
  the assertion without subsequent terminal output.
- e2e/tests/work-start-order.spec.ts:175-180 separately checks ordering, keys and connected state; none repairs the
  invalid progress baseline.
- No matching Planned item, queue item or ledger decision was identified. TRIAGE_OUTCOMES.md:3338-3355 concerns
  production ticker blocking on lifecycle claims, not this test oracle.
- review_feedback_queue/FILTER.md:41 excludes false-success consequences. This concern is therefore not disposed of by
  the rare safe-failure filter.
- No runtime interleaving was reproduced, and this does not establish a production ordering defect or user-work loss.
  Reject empty or malformed baseline reads, or publish progress atomically.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_15_cor:p1:C9`.

- `test_infrastructure_15_cor:p1:C9`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
