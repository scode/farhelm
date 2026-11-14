# deleted ambiguous-create test loses published-handle and pending-intent oracles

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: deleted ambiguous-create test loses published-handle and pending-intent oracles

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g108/c5`.

### g108/c5

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 13 (finding, open):

# retain an uncertain create's visible handle and retry identity in the host test

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513

## TLDR

The migration removes a test proving that an uncertain failed create remains visible and keeps its retry unresolved. The
retained host equivalent proves only that a database row survives. It could pass if the session disappeared from the
running supervisor or its retry were wrongly finalized. Correct current production code does not replace those removed
regression oracles.

## Details

Assigned patch lines 980–1103 remove an_ambiguous_tmux_failure_keeps_the_launching_record (old core.rs:33988 onward).
Besides retaining one Launching row, that test explicitly asserted sessions.contains_key(row.id),
ReservationOutcome::Pending for its key, and ordinary_create_was_refused == false. Those checked three distinct
consequences: durable ownership, immediate reachability of Stop/Delete/listing, and a retry that still reconciles
instead of replaying a final refusal. The actual host replacement is e2e/host_creates.rs:1254–1299,
an_unusable_host_that_cannot_be_ended_keeps_the_launching_record. Its never-answering listener has a command line the
verified signal rejects, so it meaningfully reaches failed cleanup. It asserts the 'may still be running at'/'launching
record' diagnostic, one durable host Launching row, and that the listener remains alive. But it calls
client.create_session, whose client.rs:3214–3224 explicitly passes None as intent key; it neither lists/resolves the
retained session from the running supervisor nor checks a reservation. A wrong Failed settlement for keyed uncertain
failure would be invisible, and a missing publish_retained_launch would not change this test's inspected database row or
listener. The separate keyed interruption test (host_creates.rs:886–973) deliberately exits via a DuringLaunch simulated
crash, before the FailedStart::Terminal cleanup branch, and asserts Created only after a successful retry; it does not
cover failed-cleanup publication/settlement. Targeted searches of host tests for the uncertain
diagnostic/HostLeft::MayRemain/HostSpawnFailure::Host found no second uncertain-create fixture that supplies these
missing assertions. At tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db the same retained host test is :1169–1213 and
remains unkeyed with only durable-row/listener checks. The retired tmux symlink-loop/new-session mechanism need not
return; extend the existing host fixture with a key and assert its retained session is listed/reachable, reservation
remains Pending, and ordinary-create refusal classification does not finalize it. Contract: SPEC.md:1371–1377 requires
one intended create and retained retry identity; SPEC_impl.md:2335–2342 retains uncertain hosts through launching
records. Production core.rs:11803–11818 and :11936–11961 correctly keeps the row and publishes it without Failed
settlement; no actual orphaned process or incorrect final settlement is established. Suggested bucket: other (lost
regression coverage). Confidence: confirmed source analysis; no execution/reproduction. proposed_drop: false; earlier
proposed drop based on current correct production code is superseded. possible_cover: none known; parent-supplied
g103/f1 concerns unmarked-shell cleanup, a different behavior. Could-be-highest: preserve the original concern that
discarding the visible/cleanup handle or finalizing an uncertain intent can hide/orphan running user work, but no such
current production route is established, so this finding's established impact remains test quality. Caveat: host path
coverage was already narrower in these assertions before this deletion; the regression here is removing the only
inspected cross-terminal test oracle instead of carrying its required behavior into the retained host fixture.

Returned proposal (claims-070, event 13): **retain-verified**, suggested bucket **other**.

The removed keyed ambiguous-create test checks both immediate publication and unresolved retry identity. Its retained
host counterpart drives genuine failed cleanup but is unkeyed and checks only the durable row and surviving listener.
Current correct production retention does not establish those missing oracles. Earlier proposed orphaning-route
dismissal is superseded.

Original stronger consequence: A possibly live launch could lose its visible cleanup handle or have its uncertain retry
finalized, hiding/orphaning user work. The retained production source currently protects that route; established impact
is test quality.

Unresolved premise or limitation: No missing premise for the bounded removed-oracle comparison. No current orphaned
process, missing publication or incorrectly finalized reservation is established. Host tests were already narrower
before this deletion; the change removes the inspected cross-terminal assertions instead of porting them.

Proposed basis: Retain the missing publication/Pending test oracles. Earlier production-route refutation and consequence
narrowing still need independent judgment.

Existing parent disposition (event 13): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g108 record](records/review-through-00de0b6c/groups/g108.json). Record ID: `c5`.
