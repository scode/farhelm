# Intentional child passes because it failed for the wrong reason

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The intentional teardown-failure child could also fail its body and still pass the parent.

## Details

F306 — **possible** — `e2e/harness-tests/child-runner.ts:302` — Intentional child passes because it failed for the wrong
reason

The child records body completion before its last assertion. A body failure afterward can coexist with the expected hook
failure and satisfy the parent's marker, lifecycle, and attachment checks, none of which establish a passing body. No
counterexample was executed. Move the completion marker after the final body assertion and validate the intended hook
error without preceding body failures.

## Evidence and triage context

- e2e/harness-tests/timeline.contract.ts:803-812 invokes the teardown-failure contract through verifyIntentionalChild.
- e2e/harness-tests/timeline-child.failure.ts:41-45 promises a hook failure after a passing body, but records
  outcome=teardown at :44 before awaiting the body assertion at :45.
- e2e/harness-tests/timeline-child.failure.ts:6-10 records after-each and throws the hook error based on title, without
  requiring a successful body.
- e2e/harness-tests/child-runner.ts:245-255 accepts failed/passed structured status plus any reporter error for
  teardown-failure; it does not reject a preceding body assertion error.
- e2e/harness-tests/child-runner.ts:302-303 treats the early outcome marker as proof of a successful body. The lifecycle
  checks at :281-287 can still pass after a body assertion failure.
- e2e/tests/helpers/evidence.ts:34-53 attaches evidence for the unexpected final result, including a body failure, so
  attachment existence does not establish the missing premise.
- No runtime counterexample was executed. Move the completion marker after the last body assertion and validate the
  intended hook failure without preceding body errors.
- review_feedback_queue/birth-oracle.md:14-23 concerns filesystem-capability probe failures, not this browser lifecycle
  oracle; it does not cover this candidate.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_04_sec:p1:C6`.

- `test_infrastructure_04_sec:p1:C6`: confidence as filed: not separately tagged in candidate list; suggested bucket as
  filed: not separately tagged in candidate list.
