# Tab-retarget test can lose ownership during setup

Reviewed commit: 798a3fd6c4dc672df83e74b09fbeeedcec96c209

## TLDR

If the browser test fails while creating its terminal tabs, its agent and tabs remain running for later tests.

## Details

Suggested bucket: `other`. Confidence: Confirmed static trace; not reproduced. Fixture leakage, not loss of user-owned
work. Source: dedicated `gpt-6.1-sol` high adversarial review of this commit. Proposed drop: none.

The new test awaits `openSessionWithTabs` before entering its cleanup try (`e2e/tests/terminal-tabs.spec.ts:1253,1260`).
The helper creates a live session at `535`, then navigates, attaches, creates tabs and waits for reveal at `536–542`. A
failure leaves the caller without the session ID, so cleanup at `1275–1277` never runs.

The suite's afterEach only unregisters routes, and afterAll removes fixture directories
(`e2e/tests/helpers/terminal-suite.ts:198–213`). The next spec's reset (`147–178`) eventually removes sessions, but they
retain processes and pollute the list meanwhile; the directory sweep can remove their working directories while they
remain alive.

Give the helper failure cleanup after creation, or give the caller ownership before fallible setup.

Possible cover: TRIAGE_OUTCOMES.md heading `busy-selection-test-loses-source-second-creation-fails.md` covers an
analogous different fixture, not this added test.

Current-main status: persists by inspection at `4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703`; no fixing commit found. The
reviewer read the full commit diff and relevant traces; this is not an exhaustive review of unchanged surrounding code.

## Additional affected scope

Reviewed commit: 798a3fd6c4dc672df83e74b09fbeeedcec96c209

## TLDR

A failed browser-test setup can leave its agent and shell tabs running until a later stack reset or shutdown.

## Details

Suggested bucket: `other`. Confidence: confirmed by control-flow inspection. Source: dedicated `gpt-6-astra` high
adversarial review. Proposed drop: none.

The new test calls openSessionWithTabs at e2e/tests/terminal-tabs.spec.ts:1253 and installs its route at 1256–1259
before entering the cleanup try/finally at 1260–1276. The helper creates a real session at 535, then performs
navigation, attachment, tab creation and readiness waits at 536–542 without failure cleanup. Any failure after creation
but before return loses the session ID to the caller and skips cleanup; route-registration failure also skips cleanup
after successful setup. terminal-suite.ts:196–214 only drains routes after each test and removes scratch directories
after the file; it does not stop these sessions. Establish ownership immediately after creation and clean up failed
setup within the helper, then include route registration inside the caller's protected lifetime. The helper weakness
predates this commit; this newly added test introduces another affected caller.

Possible cover: TRIAGE_OUTCOMES.md heading busy-selection-test-loses-source-second-creation-fails.md describes the same
ownership mistake in a different sidebar test; its scope does not cover this new test.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703; the test and relevant helpers are
unchanged. No fixing commit.

Review coverage: full commit diff read: true. No commit diff hunks remain unread. Surrounding modules and test files
were read selectively, not in full. The 13,089-line triage ledger was searched for relevant topics and matching sections
read; unrelated ledger sections and individual queue findings with no matching INDEX topic were not read. No Sol reports
or working-copy queue additions were read. Overlapping-dialog behavior was not reproduced.

Runtime evidence: None, as requested by default. Review was read-only; no checkout or scratch files were created,
modified or deleted. No fetch, rebase or helper agents.
