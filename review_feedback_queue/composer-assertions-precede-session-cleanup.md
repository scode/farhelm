# Composer assertions precede owned-session cleanup

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A failed composer assertion can leave its already-created fake session running on the shared test stack.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The composer-word-search cases acquire sessions before entering the cleanup scope that knows their IDs. An assertion
failure can skip that registration. Eventual worker or stack teardown does not establish prompt cleanup of this owned
fixture. No production session or user-work loss is claimed.

Source locations: `e2e/tests/composer-word-search.spec.ts:161-174,329-339`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed full commit d6645f771bee8ed03a66486ef8755834937d759d; inherited at cc734789f2ce77155ad2838eea7771bd52fe240c,
not introduced by assigned commit 08aaac8a4a5bcf91726a1d3bfcfc980de4116971. TLDR: when either successful-launch test
fails after creating a session, its fake agent/session can remain on the shared test stack instead of being reclaimed,
adding state for subsequent checks. In the structured test a mismatching persisted selection times out at lines 162-172
before cleanupSession at 174. In the command test a request-body assertion at 336-337 throws before response ID
extraction and cleanup at 338-339. Neither test has finally/afterEach ownership. Source checks:
helpers/evidence.ts:31-65 adds only timeline/context instrumentation, not session deletion; helpers/host-picker.ts:1-30
and fleet.ts imports:16-21 add no test teardown; playwright.config.ts:58-75,98-107 reuses stack state across serial
tests/projects. The file does not install terminal reset hooks, and those hooks reset only on entry to a participating
spec (terminal-suite.ts:154-194,203-222). Client teardown does not end sessions (SPEC.md:28-39). Fake Codex construction
at e2e/start-stack.sh:226-254 and FAKE_AGENT at helpers/fleet.ts:155-167 confine this to owned test work. Confidence
confirmed conditional source path for leaked session after post-allocation assertion failure; no failure was injected,
and downstream false-pass/flake frequency is unmeasured and not claimed. Suggested bucket other; no potentially-highest
route. A fix should register each created session for cleanup as soon as its ID is known and run that cleanup in
finally; the structured test also needs ownership before asserting selection correctness. Coverage search included
SPEC.md, SPEC_impl.md, empty TODO Planned bucket (23-24), BUGS.md, plans/queue, queue INDEX/referenced cleanup file, and
TRIAGE_OUTCOMES.md. No exact coverage found. possible_cover: catalog-cleanup-can-start-untracked-test-session.md and
TRIAGE_OUTCOMES.md:20197-20209 concern future creates after observation ends in four named specs; this is
already-allocated sessions in composer-word-search.spec.ts. tab-retarget-fixture-escapes-cleanup.md
(TRIAGE_OUTCOMES.md:14241-14264) and notification-fixtures-register-ownership-too-late.md (17826-17845) are completed
repairs confined to other tests. These are precedents, not coverage. proposed_drop: none. Log ID f1. No runtime results
or external approval are authenticated.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g028/f1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
