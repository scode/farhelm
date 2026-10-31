# Creator revocation test does not witness its wait boundary

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The delayed-creator revocation regression can release its gate without proving the creator actually reached the queued
lock boundary.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The inspected test uses a proxy observation rather than the actual wait/admission boundary it claims to exercise.
Production ticket checking under the lock is correct in the checked path. This is a test-oracle gap, not evidence of a
production revocation bypass.

Source locations: `crates/farhelm-supervisor/src/container.rs:1188-1201`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed d6645f771bee8ed03a66486ef8755834937d759d. TLDR: the regression test can pass without exercising a creator
already waiting when teardown revokes creation, so it can miss a regression in the ordering that prevents late
recreation. Confirmed source-only test-oracle gap: parent acquires the guard (1195), spawns child (1196), immediately
revokes (1197), then drops guard and observes failure/absent fake state (1198-1201). No child-ready/lock-attempt witness
precedes revocation. A change that reads the creation ticket before acquiring the lock would still pass if the child is
first scheduled after revocation; the same incorrect implementation can fail under another schedule. No experiment or
observed false pass is claimed. Production is presently correct: prepare acquires the flock before reading its ticket
(572-576). All container module tests through line 1350 were inspected. The positive hangup test (1149-1185) witnesses
lock ownership across a running child, not ticket-read ordering for an already-waiting creator. Generation mismatch
(1204-1215), cleanup/absence and pending-marker tests (1218-1348) likewise do not establish that boundary. The
real-container e2e symbol search found no creator-ticket/flock revocation oracle. Suggested bucket other, confidence
confirmed for the missing boundary witness only; no production security/work-loss consequence alleged, and no highest
classification requested. A meaningful repair would provide an owned readiness witness for the intended pre-revocation
waiting boundary rather than adding an observation delay. proposed_drop: none. possible_cover: none found.
SPEC.md:3648-3652 and SPEC_impl.md:5782-5794 state the creation/teardown guarantee, not acceptance of missing test
evidence. Exact coverage search across both specs, TODO Planned (empty), unlanded-plan index (empty), BUGS, queue index
and triage ledger found no matching finding or accepted omission. The generic queue shutdown-wait item concerns another
test/operation and is not trigger/consequence/scope coverage. The committed-code filters do not exactly cover this
durable test-evidence gap.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a050/f10`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
