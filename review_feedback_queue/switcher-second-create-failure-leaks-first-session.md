# Switcher setup can leak its first acquired session

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

If a second setup create fails, the switcher test can skip cleanup of the first session it already acquired.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The first acquisition occurs outside the cleanup scope entered only after the second allocation. Eventual shared-stack
reset bounds the leak but does not make that test's resource ownership correct. No production behavior defect is
established.

Source locations: `e2e/tests/quick-switcher.spec.ts:94-103;206-208`. Historical revision qualifiers in those references
are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d; assigned patch commit
2447296f34f256f239a36e5a6faecec5ca028f1d. This is pre-existing in the assigned test file, not introduced by label
casing. TLDR: a failed switcher-test setup can leave a fake-agent session on its shared browser-test stack, retaining
resources and changing later tests' fleet until a later explicit suite reset or stack teardown. This refines the earlier
event's overly broad until-stack-teardown wording: terminal-suite.ts:154-194 can delete leftovers when a later
terminal-family file invokes resetStack in beforeAll (203-204). quick-switcher.spec.ts does not install that hook. The
Mac/non-Mac chord case creates first and second at 94-95 before its cleanup try begins at 103 (finally 190-195); the
hidden-host case repeats it at 206-208 (finally 223-226). If the second create rejects or returns non-success, the first
successful allocation never reaches cleanup. fleet.ts:396-423 creates and returns without registering cleanup, and its
ok helper at 328-334 throws on non-success. The spec's afterEach at 33 only resets preferences (fleet.ts:1115-1150);
evidence.ts:31-65 adds observation only. playwright.config.ts:58-73,98-117 confirms long-lived shared stacks across
serial tests/projects; browser/worker replacement does not recreate the stack. Register each acquired ID under cleanup
before another allocation can fail. Suggested bucket other; confidence confirmed conditional source path. Actual failure
frequency and downstream test failures are unmeasured; no runtime reproduction or production user-work-loss/security
consequence is claimed. proposed_drop false. possible_cover: no exact match found in SPEC.md, SPEC_impl.md,
TODO.md/Planned, BUGS.md, plans/queue, queue index and referenced cleanup item, or TRIAGE_OUTCOMES.md. Ledger
12515-12525 covers sidebar tests, 17826-17845 notification tests, and 14705-14726 a different quick-switcher template
setup. catalog-cleanup-can-start-untracked-test-session.md concerns later async allocation after observation ends, not
these already acquired sessions. Those scopes do not remove this cause. No highest-bucket exclusion is proposed.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g060/f1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
