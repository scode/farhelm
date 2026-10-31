# Zoom cleanup failure skips a second owned session

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A cleanup refusal can leave the zoom/remount test's remaining session running until wider teardown.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The fallible first cleanup short-circuits the remaining owned-session cleanup. Playwright worker retirement does not
delete a shared-stack session, so eventual wider teardown does not refute this temporary fixture leak. No concrete later
false failure or product user-work loss was established.

Source locations: `e2e/tests/terminal-tabs.spec.ts:1623-1625`. Historical revision qualifiers in those references are
retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d; introduced by afd0fe73c144156fb24db96ac782268e4b639e7d. TLDR:
a failed cleanup in the terminal-size regression leaves its other fake-agent session alive in the shared test stack,
even if that other session could be cleaned successfully. Details: the test records two successfully created IDs at
:1590-1593, then finally loops with an unguarded await at :1624. helpers/term.ts:179-190 throws on a non-404 Stop/Delete
refusal or transport exception, so the first such failure prevents any cleanup request for the second ID.
installTerminalSuiteHooks({tabSweep:true}) (:39) provides only route draining and afterAll removal of tabSessionDirs,
not session deletion (helpers/terminal-suite.ts:203-220); createTabSession registers both directories (:415-431).
Matching Playwright 1.64.0 worker source (parent-supplied
g040/playwright-source/package/lib/worker/workerProcessEntry.js:1662-1721) runs afterAll then stops the failing worker.
Thus the second fake-agent session can outlive the worker while its cwd is removed. Framework runner source
:5795-5813,6079-6137 requeues remaining tests and :5851-5860 passes each worker slot index into a replacement worker;
repository stack-slots.ts:87-107 maps that index to the same long-lived helm. The webServer belongs to run-level
setup/teardown (runner/index.js:6542-6555,7127-7153 and :6315-6377), not worker failure. Impact is bounded but real: the
skipped session/processes and session row remain in the private shared stack until a later terminal-family reset on that
stack (helpers/terminal-suite.ts:154-185) or final server shutdown. A replacement terminal worker on the same slot
resets before testing, so no specific downstream false failure is claimed; if jobs move to another free slot the old
stack remains until its own reset/shutdown. Existing cleanupOwnedSteps at helpers/cleanup.ts:6-19, already imported by
this spec at :16, supplies an independent-attempt pattern without changing cleanupSession's within-session policy.
Suggested bucket other, confirmed conditional source mechanism; failure frequency and downstream timing were not
reproduced. proposed_drop: none. possible_cover: analogous historical decisions
TRIAGE_OUTCOMES.md:12446-12465,17182-17216 concern named older tests, not this new regression.
checkout-test-skips-new-spare-setting-restoration.md records the shared orchestration class but different preference
consequences; ledger :20338-20351 limits its own execution and does not explicitly cover this instance. Search of
SPEC.md, SPEC_impl.md, TODO Planned (empty), BUGS.md, plans/queue, queue INDEX and ledger found no exact coverage. No
FILTER matches the complete resource-retention consequence. No highest route: private fake-agent fixture resources only.
Source-only, no runtime or private report evidence authenticated. Original reopening/withdrawal history preserved;
initial eventual-shutdown-only withdrawal was insufficient.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g110/f2`, `g129/f1`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
