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

## Trailing review through ca9b0a19

Failure cleaning one fake-agent test session prevents any cleanup attempt for the other.

Suggested bucket: **extend**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source path; no measured frequency or survivor duration. Fix: attempt both owned cleanups independently and aggregate
errors.

Source locations: ca9b0a19856dc136f63ad2d14f1076027edce1eb:e2e/tests/terminal-migration.spec.ts:206-208.

terminal-migration.spec.ts:191-193 awaits cleanupSession(neighbor) before cleanupSession(session), without an
independent-attempt guard. helpers/fleet.ts:559-569 throws on transport failure or a non-404 refused response, so the
source session is not attempted in that case.

terminal-migration.spec.ts:191-193 awaits cleanupSession(neighbor) before cleanupSession(session), without an
independent-attempt guard. helpers/fleet.ts:559-569 throws on transport failure or a non-404 refused response. Even when
source cleanup would succeed, no request for it is made after neighbor cleanup rejects. Later resetStack
(helpers/terminal-suite.ts:154-194) bounds retention, not this test's cleanup ownership.

failure cleaning one fake-agent test session prevents any cleanup attempt for the other. The tip still awaits
cleanupSession(neighbor) at 207 before cleanupSession(source) at 208. Tip fleet.ts:559-569 throws on transport failure
or non-404 refused response, including a refused Stop whose Delete may already have succeeded. A source cleanup that
could succeed is never attempted. Later stack reset bounds retention but does not execute this skipped caller step.
Bucket: other.

Possible coverage and caveats: none exact. Full queue zoom-cleanup-failure-skips-second-session.md targets
terminal-tabs.spec.ts; sort-test-cleanup-stops-after-first-failure.md targets sort.spec.ts. Ledger trust and listing
ownership sections at 14073-14102 and 14683-14703 prescribe fixes in those named files, not migration.spec.ts. Coverage
searches of SPEC.md, SPEC_impl.md, empty TODO Planned, plans/queue, BUGS.md, queue INDEX and ledger found no exact
coverage. FILTER safe-failure/status/dependency-hang rules do not cover this inter-session test resource retention.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g193/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb;
introduced in assigned patch 4ed000fb454cebd9c22a88f358a93e15566b8c9e at e2e/tests/terminal-migration.spec.ts:191-193..
This was a source-only review; runtime occurrence and frequency were not measured.
