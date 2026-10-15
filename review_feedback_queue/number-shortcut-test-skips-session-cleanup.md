# Expanded shortcut tests can lose cleanup ownership of their sessions

Reviewed commit: 4fced22e4d9bc501bbb84948a77cd52ef26203d6

## TLDR

A failed number-shortcut browser test can leave its agent session running and affect later tests.

## Details

Suggested bucket: `other`. Confidence: confirmed static failure paths; not reproduced. Source: separate adversarial
trailing-review agent, templates scope. Proposed drop: none. Caveat: the cleanup defect existed in the earlier quick
launch test; this commit expands that body to two number-key cases.

`e2e/tests/quick-launch.spec.ts:529–556` creates the source session at `535`, then performs fallible route registration
at `548` before entering `try`. A failure there bypasses source cleanup. Inside `finally` at `608–616`, failures in
unroute, browser evaluation or an earlier created-session cleanup also skip source cleanup. The spec's `afterEach` only
restores preferences. `helpers/evidence.ts:31–65` collects evidence rather than reclaiming sessions; this spec does not
install the terminal suite's broader reset hooks.

Acquire cleanup ownership immediately when creating the source and independently attempt every owned cleanup. Inject a
route-registration or early-cleanup failure to verify source deletion still occurs. No shipped-product data loss was
established.

Possible cover: `template-regression-fixtures-arm-cleanup-too-late.md` concerns template fixtures in other tests, and
`header-regression-fixtures-escape-cleanup.md` concerns header-test sessions. Neither covers this test's resources and
trigger. No exact Planned item, accepted bug, ledger decision or filter was found.
