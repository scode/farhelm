# The new paste regression acquires fixture cleanup ownership after readiness

Reviewed commit: dd9573cc07782949917597520bdc49ca84706cc0

## TLDR

A failed browser-test setup can leave its fixture agent running and then remove its working directory.

## Details

Suggested bucket: `other`. Confidence: confirmed by inspection; failure path not executed. Source: dedicated
`gpt-6.1-sol` high adversarial review. Proposed drop: none.

e2e/tests/terminal-attachments.spec.ts:673–674 assigns the cleanup id only after openAttachmentSession returns. That
helper creates the session at :459, then awaits navigation, row selection, revealed readiness and the agent banner at
:460–463. Any failure after creation leaves id undefined, so :684–685 skips cleanup. helpers/terminal-suite.ts:413–424
registers only the directory; its afterAll at :207–213 removes directories without deleting these sessions. Its session
sweep runs before the file at :197, not after a failed test. Acquire cleanup ownership immediately after creation,
before readiness awaits, or make the helper clean up on failure. This is a concrete fixture-lifetime violation of the
supplied test checklist; the helper pattern predates the commit, but the added test introduces another affected fixture.

Possible cover: review_feedback_queue/header-regression-fixtures-escape-cleanup.md and
tab-retarget-fixture-escapes-cleanup.md concern other fixtures; they do not cover this added attachment test.

Current-main status: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703. The added test, openAttachmentSession and
suite cleanup hooks retain the affected ownership ordering.

The full commit diff and relevant paths were read. No runtime reproduction was run.
