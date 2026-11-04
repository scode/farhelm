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
