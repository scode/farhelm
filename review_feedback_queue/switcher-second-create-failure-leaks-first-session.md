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

## Trailing review through ca9b0a19

A failed second setup create can leave the migration test's first fake-agent session running until a later shared-stack
reset or teardown.

Suggested bucket: **extend**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source path; no measured flake, survivor lifetime, downstream contamination or production user-work loss claimed. Fix:
protect the first acquisition immediately, track the neighbor as optional inside that lifetime.

Source locations: ca9b0a19856dc136f63ad2d14f1076027edce1eb:e2e/tests/terminal-migration.spec.ts:149-151.

the old-session band test creates session, then awaits createSession for neighbor before entering try/finally. If
neighbor creation rejects, cleanupSession for the first returned session is never reached. Source: p0707.diff lines
142-145 (new-file lines 136-139); final cleanup at new-file lines 196-199 cannot run before try begins.

terminal-migration.spec.ts:137-139 creates session and then awaits neighbor before try. helpers/fleet.ts:396-423 returns
the created row without registering cleanup and throws on failed creates via ok at 331-334. Migration spec imports only
evidence/fleet/term and installs no per-test session sweep; evidence.ts:31-65 observes only. Later terminal-family
resetStack can reclaim leftovers (helpers/terminal-suite.ts:154-194), but this does not attempt cleanup on this failure
path.

a failed second setup create can leave the migration test's first fake-agent session running until a later shared-stack
reset or teardown. At tip, migration test lines 149-151 still await neighbor creation before entering try;
fleet.ts:396-423 returns an ID without registering caller cleanup, and its ok helper at 331-334 throws on unsuccessful
responses. Test-local cleanup is therefore bypassed when the second create fails. Bucket: other.

Possible coverage and caveats: none exact. Rechecked switcher-second-create-failure-leaks-first-session.md in full: its
trigger/consequence match but scope is explicitly quick-switcher.spec.ts, not this new caller. Ledger
busy-selection-test-loses-source-second-creation-fails.md:12515-12525 concerns sidebar.spec.ts;
claude-trust-browser-fixtures-escape-cleanup.md:14073-14102 expressly confines the fix to claude-trust.spec.ts;
listing-regression-fixtures-escape-cleanup.md:14683-14703 concerns listing-mutation-fence.spec.ts. Existing decisions
support fixing ownership but do not remove this cause. Coverage searches of SPEC.md, SPEC_impl.md, empty TODO Planned,
plans/queue, BUGS.md, queue INDEX and ledger found no migration-test coverage. FILTER rare-safe-failure does not cover
retained running fixture resources.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g193/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb;
introduced in assigned patch 4ed000fb454cebd9c22a88f358a93e15566b8c9e at e2e/tests/terminal-migration.spec.ts:137-139..
This was a source-only review; runtime occurrence and frequency were not measured.

## Abbreviated review through 00de0b6c

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g026/f6`, `g069/l1`.

### g026/f6

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 27 (finding, open):

Reviewed commit: 086d4ff96efb13bdb51accaa1ce0a0a6f7d5ca5b

## TLDR
If route setup fails after this test creates its source session, the test exits without deleting that session. Later tests on the shared stack can inherit the extra session and agent. This affects maintainer test isolation, not a user's installation.

## Details
The assigned new test creates session at :461, awaits drainedRoute at :468 and :489, and enters try only at :493. Its only cleanupSession call is inside the finally at :512. drained-route.ts:36 awaits page.route and propagates failure before returning a stop function, so registration failure after the successful real create bypasses this finally. createSession in helpers/fleet.ts:408-438 posts a real /api/sessions request using FAKE_AGENT by default and returns the durable session ID; browser/page disposal is not that session's lifetime. helpers/evidence.ts:46-112 tears down timeline, observational premise tracking and browser context but never deletes sessions. This spec has no enclosing per-test owned-session sweep. The configured stack is long-lived and shared across serial tests and files (playwright.config.ts:119-134,160-169). Thus a rejected/cancelled route registration, including test timeout while setup awaits it, can leave the created session past this test's cleanup boundary. The enclosing stack eventually ends; no indefinite user-process leak or catastrophic tooling harm is claimed.

Move ownership acquisition and all later fallible setup under the existing try/finally, registering the returned ID immediately. A clean normal run and assertion failures after :493 already clean up; the gap is specifically the pre-try setup window. The test-authoring contract requires owned fixture lifetime/failure paths to remain established. Suggested bucket: other. Could-be-highest: false. Confidence: confirmed source-level failure path, not an observed failure. proposed_drop: false. possible_cover: neighbour g069/l1 describes the same cleanup-ownership pattern in podman-environments.spec.ts; this record identifies the separately assigned newly added caller and may be grouped with that common remediation by the parent. It is distinct from g026/f5's raw wait inside an entered try. No runtime tests, reproductions or live-install interaction.

Discovery continuation checked the pinned endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db: the same test still creates its session at :476, awaits the two route registrations at :483 and :504, and enters try only at :508. The pre-try ownership gap therefore persists; the later UI/terminal changes do not move cleanup admission. No runtime observation added.

Citation correction after numbered helper read: the awaited page.route call is drained-route.ts:38, not :36. Its propagation/ownership conclusion is unchanged.

Reopened synthesis turn 5: supplied g024/l5 does not remove this separate pre-try cleanup obligation. ownedHost chooses a real connected ordinary SSH host and rewrites its browser-visible ownership/state; the created source is an ordinary real session, not a real stopped Podman actor. The shared stack is intentionally alive across tests, with no per-test manager drop to trigger either the documented manager fallback or this newly supplied resource-retention path. Ordinary global teardown later ending the stack does not undo the missing cleanup before subsequent tests run. Original pre-try consequence, other bucket, possible-cover note and proposed_drop:false unchanged. No new production or runtime claim.

Returned proposal (claims-015, event 21): **retain-verified**, suggested bucket **other**.

A rejected route registration after the real session create bypasses the only session cleanup. This is a conditional per-test isolation defect, not an observed leak or product failure.

Original stronger consequence: No highest consequence was asserted: extra test-owned session/agent may survive this test until broader stack cleanup. No indefinite user-process leak or catastrophic tooling harm.

Unresolved premise or limitation: Actual leftover work requires route-registration rejection/cancellation after create succeeds; incidence and downstream failures were not observed.

Proposed basis: Record the conditional setup ownership gap, preserving its bounded maintainer-test scope. Parent-supplied g069/l1 is related provenance only; its source/history was not assigned here.

Existing parent disposition (event 21): duplicate. Parent revises consolidation only after reading claims043 g069/l1 and the complete existing acquisition-before-guard item. Previously verified g026/f6 remains source-confirmed:00de:tensorlake-sandbox-sessions474-508 acquires source before awaited route registration can reject and before try; cleanup523-528 cannot run. Same root as g069/l1's newly checked Podman callers, with full independent provenance retained. Extend the existing item with both scopes instead of creating another acquisition-before-guard record. Not a refutation, confidence lowering, existing exact cover or discard of caller evidence; bounded test-stack contamination only.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g026 record](records/review-through-00de0b6c/groups/g026.json). Record ID: `f6`.

### g069/l1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 7 (finding, open):

Reviewed commit: 350a4b33b6d4476f4a78e43cea124bc2b828f462

## TLDR
If setup fails after creating the source session, this browser test leaves its fixture session in the shared test stack. Later tests can inherit the extra session and its agent instead of a restored fixture. A saved-template fixture has the same cleanup gap. This affects test isolation, not a user's installation.

## Details
The active-placement test creates source at lines 111-114, then awaits a listing and asserts its success, source presence and host at lines 115-119. Its try/finally begins only at line 245; cleanupSession(source.id) is inside that finally at line 292. A refused listing, missing source row or failed route registration between creation and line 245 bypasses cleanup entirely. The same pre-try ownership gap occurs for source creation at lines 501 and 555. The quick-launch test likewise saves its real template at lines 689-702 but does not enter try until line 741, after more awaited route registration.

The source-session consequence is supported by e2e/tests/helpers/fleet.ts:408-441 (real API create) and 576-589 (explicit stop/delete cleanup). Its FAKE_AGENT definition at lines 165-175 selects the basic fixture; crates/farhelm-fixtures/src/fake_agent.rs:1675-1697 shows the fixture waiting for stdin until quit, rather than exiting at browser disposal. e2e/tests/helpers/evidence.ts:46-112 tears down timelines, premise observation and browser contexts without deleting API sessions. e2e/playwright.config.ts:119-134 describes a long-lived shared stack, and lines 160-169 require specs that disturb shared stack state to put it back. A failed browser worker/context is therefore not an owned-session cleanup boundary. Put session/template creation and every later setup operation inside the existing try/finally, recording each owned identity immediately after creation succeeds.

Suggested bucket: other; the established consequence is failure-path contamination of maintainer test tooling. Confidence: confirmed from source and fixture lifecycle contracts; no runtime execution or reproduction. proposed_drop: false. Could-be-highest: false; only the harness's owned fixtures are affected, and no catastrophic tooling consequence is established. possible_cover: none. Caveats: a setup failure is required; normal success and failures inside the existing try clean up. The enclosing stack eventually shuts down. No cascade or indefinite process leak beyond stack shutdown was observed. This revision corrects the cleanup line reference, confirms the fake-agent lifetime from implementation, and includes the saved-template instance of the same ownership gap. The original suspected lingering-agent and later-test contamination consequences are retained.

Returned proposal (claims-043, event 7): **retain-verified**, suggested bucket **other**.

Conditional setup failure can bypass all local cleanup after a real session or template has already been acquired. Shared browser/worker teardown does not delete these API-owned fixtures. This is useful additional scope for the existing acquired-before-guard item, not exact coverage of these tests.

Original stronger consequence: Setup failure can leave a fixture agent running and contaminate later tests. Latest history adds the saved-template fixture. Normal success and failures after guard entry clean up; wider stack reset/shutdown eventually reclaims fixtures. No user installation, security or catastrophic tooling impact is claimed.

Unresolved premise or limitation: Actual failure incidence, survivor lifetime until a later suite reset, and downstream test failures were not measured. They are not required for the conditional ownership gap.

Proposed basis: Retain all source/session/template instances and caveats. Extend the acquired-before-guard queue item with this separately named scope, or keep a dedicated item if its expanded title would be misleading. Do not mark the new scope covered merely because other tests have the same ownership mistake.

Existing parent disposition (event 7): verified. Parent read full claims043 and existing acquired-before-guard item. New Podman tests at350a4b33:111-119,501,555,689-702 acquire real session/template state before fallible assertions or route setup and before their cleanup guards. This is useful additional caller scope for the same ownership gap. Existing item covers quick-switcher/migration only, so extend it; not an accepted dismissal. Real API agents outlive browser disposal, later stack reset bounds residue, no actual incidence or user-work loss claimed. Current coverage searches found no acceptance/pending repair. Reconcile g026/f6's same pre-guard acquisition mechanism here, preserving each caller and distinct fallible setup.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g069 record](records/review-through-00de0b6c/groups/g069.json). Record ID: `l1`.

