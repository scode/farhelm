# An owner-death test spawns its child before cleanup registration

Reviewed commit: 3efb0380099bd695c092eeac3be2e21cead4d70a

## TLDR

Killing the test owner before it publishes the child's PID can leave its owned five-minute sleep outside cleanup. This
is separate from the explicitly accepted same-run PID-reuse limit.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location:
`crates/farhelm/tests/spare_builder.rs:199-215,229-235,580-589,618-623,690-693@3efb0380099bd695c092eeac3be2e21cead4d70a`.
Locations and revisions in the assessment below identify the reviewed source; later main is outside this review's pinned
range.

Later-turn synthesis for producer g267/f2. Reviewed commit 3efb0380099bd695c092eeac3be2e21cead4d70a. TLDR: failure while
waiting for the owner to publish its child's PID can strand the test-owned sleep for the rest of its 300-second
lifetime. Confirmed conditional source mechanism; unmeasured scheduling frequency. Suggested bucket other, no
established route to user-owned processes. The actual patch fixes waits after publication, but cannot arm Indirect
before await_unrelated_child returns; the owner can be killed by parent unwind in the earlier spawn-to-publication gap.
The original deleted queue item at the reviewed commit's parent,
review_feedback_queue/spare-owner-tests-register-child-cleanup-after-readiness.md 15-27, described the broader
failure-before-registration class and explicitly distinguished ownership gaps from PID-reuse acceptance. Pinned ledger
17887-17906 chooses a narrower implementation: wait for the child's PID, arm ownership, then wait for other readiness.
That criterion is met; it does not explicitly accept a child stranded before PID publication. Verdict on any full
exclusion by that ledger is AMBIGUOUS, so preserve this remainder; verdict on applying same-run PID acceptance is
DISAGREE. possible_cover is partial prior ledger/queue coverage of the same tests and general cleanup consequence; do
not silently duplicate or treat it as newly introduced spawn order. proposed_drop none. Focused searches across pinned
SPEC.md, SPEC_impl.md, TODO.md (Planned empty from prior read), BUGS.md, INDEX.md and TRIAGE_OUTCOMES.md found no other
exact coverage; macOS-only e2e cleanup acceptance at ledger 12580-12601 has a different trigger/scope, and the queued
detached replacement-supervisor publication item has a different fixture. No tests, reproductions or experiments.

Provenance: `a043/f3`, `g267/f2`. These are reviewer-local identities, not additional findings. Proposed drop: none for
the retained remainder.
