# Listing regression test can skip cleanup of its source session

Reviewed commit: 7522d172fce0699cd5e85547a0b536eb821bc13d

## TLDR

A failed browser test can leave a fixture agent running and affect subsequent tests.

## Details

Suggested bucket: `other`. Confidence: confirmed by static failure-path tracing; not reproduced. Source: dedicated
`gpt-6.1-sol` high adversarial review. Proposed drop: none.

e2e/tests/listing-mutation-fence.spec.ts:13 creates the source before fallible setup at :17–39 and before entering try
at :40. A setup rejection therefore skips source cleanup. Separately, :82–83 awaits replacement/new-session cleanup
before source cleanup: if the first cleanup throws, the source cleanup is never attempted. This matters particularly in
the create variant, where the source remains alive. helpers/fleet.ts:520–530 confirms cleanupSession throws on transport
errors or non-404 refusals. This new file has no fallback session sweep; helpers/evidence.ts:31–65 only collects
evidence. Arm cleanup immediately after acquisition and attempt each acquired session's cleanup independently. Scope is
private test fixtures, not user-owned processes.

Possible cover: review_feedback_queue/header-regression-fixtures-escape-cleanup.md and TRIAGE_OUTCOMES.md heading
history-fixtures-policy-restoration-skip-remaining-cleanup.md describe analogous failures in different fixtures. Neither
covers this new file.

Current-main status: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703,
e2e/tests/listing-mutation-fence.spec.ts:13–40,79–83. The file has no subsequent modifying commit; cleanupSession
retains its throwing behavior. No fixing commit found.

The full commit diff and relevant failure paths were read. No runtime reproduction was run.
