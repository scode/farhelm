# Header regression test can leak sessions after setup or cleanup failure

Reviewed commit: e58aafe5a8afe97d7bc7672fe4519eddcf4044e2

## TLDR

A failed browser test can leave fixture sessions in the shared test stack, affecting later tests.

## Details

Suggested bucket: `other`. Confidence: confirmed by static failure-path tracing; not reproduced. Source: dedicated
`gpt-6.1-sol` high adversarial review. Proposed drop: none.

New e2e/tests/ui-plan-regressions.spec.ts:14 awaits createResumableSession before entering its cleanup try at :21. That
helper creates the session, obtains its ID, then performs a fallible readiness poll before returning it
(e2e/tests/helpers/fleet.ts:427–449). If that poll throws or times out, neither helper nor caller deletes the created
session. This file registers no fallback session sweep; helpers/evidence.ts:31–65 collects evidence only. There is
another ownership gap at ui-plan-regressions.spec.ts:50–53: failure cleaning the source skips cleanup of the acquired
replacement. cleanupSession can throw on transport errors or non-404 refusals (helpers/fleet.ts:520–530). Establish
cleanup ownership immediately after creation, including inside the resumable helper's readiness phase, and attempt each
acquired session's cleanup independently. These are fixture leaks, not loss of user-owned work.

Possible cover: TRIAGE_OUTCOMES.md headings busy-selection-test-loses-source-second-creation-fails.md and
history-fixtures-policy-restoration-skip-remaining-cleanup.md address analogous different fixtures.
review_feedback_queue/tab-retarget-fixture-escapes-cleanup.md concerns another test. None covers this newly added test.

Current-main status: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: ui-plan-regressions.spec.ts:14–53 and
helpers/fleet.ts:427–449 retain these paths. The test file has no subsequent modifying commit; no fixing commit found.

Review coverage: the full commit diff and relevant failure paths were read. No product runtime tests were run; unchanged
surrounding code was inspected only where relevant.
