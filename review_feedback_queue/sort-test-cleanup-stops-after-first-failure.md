# Sort-test cleanup stops after its first failure

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

One cleanup failure can leave the sort test's remaining owned sessions running until wider teardown.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The afterEach cleanup loops over owned sleep fixtures through fallible operations without independently continuing after
one rejection. This concerns test-owned resources and temporary shared-stack contamination, not production user-work
loss.

Source locations: `e2e/tests/sort.spec.ts:232-236`. Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Separate discovery pass. Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d. Assigned commit
8f894cc81db3faff836763e7e03a872cabfbcc20 changes text selectors in this test file; the cleanup defect already exists at
that commit (same lines 232-236), and is not caused by sentence casing. TLDR: one failed cleanup can leave the ordering
tests' other fixture sessions running or listed on the private shared browser-test stack, affecting subsequent tests.
Details: the afterEach loop pops an ID and awaits cleanupSession without catching failure. helpers/fleet.ts:559-568
throws on a failed request or a non-404 unsuccessful response. For a non-404 failed Stop response, it can even throw
after its Delete request succeeds, still skipping other owned sessions. threeOrderedSessions records three real
sleep-300 sessions immediately after creation at sort.spec.ts:283-307. sort has no afterAll cleanup, and beforeEach at
226-228 resets only preferences. The stack is shared across projects (e2e/playwright.config.ts:58-73,98-107); sort
explicitly does not reset its stack (sort.spec.ts:175-177). Parent-provided Playwright 1.64.0 source at
g040/playwright-source/package/lib/worker/workerProcessEntry.js:1640-1733 runs afterEach, catches its failure, and
proceeds to fixture/worker teardown rather than restarting the interrupted loop. The source package reports version
1.64.0, matching e2e/package-lock.json:13-20,42-61; package cryptographic provenance was not independently checked in
this pass. Thus the remaining private session IDs are not cleaned by this hook; later stack-wide reset or shutdown can
clear them, and sleep commands terminate after 300 seconds, but their rows can persist meanwhile. Suggested bucket:
other. Confidence: confirmed conditional source mechanism; no failure injection or natural occurrence measurement, and
no claimed security or product user-work consequence. Suggested repair: independently attempt every recorded session
cleanup and report collected failures, using the existing cleanupOwnedSteps shape. proposed_drop: none. possible_cover:
no exact match. Coverage searches included SPEC.md/SPEC_impl.md, empty TODO Planned bucket, BUGS.md, unlanded plans,
queue index/files and TRIAGE_OUTCOMES.md. Ledger pin-test-skips-owned-session-cleanup.md (14542-14567),
sound-tests-skip-remaining-cleanup-and-preferences.md (15169-15193), and header-regression-fixtures-escape-cleanup.md
(14959-14982) are precedents confined to other specs, not coverage of sort.spec.ts. TODO.md:410-417 concerns
github-checkouts.spec.ts, also not this scope. FILTER.md rare diagnostic/self-correcting filters do not fully cover
remaining live fixture processes and session rows. Provenance: g055/f1, first logged immediately on discovery; this
event refines the original, without erasing it.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g055/f1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
