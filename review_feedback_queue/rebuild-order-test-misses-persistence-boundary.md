# Rebuild order test misses persistence boundary

Reviewed commit: 744ea4122d033ef616454fe592ecb2173593736c

## TLDR

The rebuild-order regression observes calls without proving the database state was persisted at the boundary it claims
to protect.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location:
`crates/farhelm-helm/src/tensorlake/build_tests.rs:527-547 at 744ea4122d033ef616454fe592ecb2173593736c`.

Source assessment (read with the later corrections below) (review record g070/d2, event 24):

Reviewed full hash 744ea4122d033ef616454fe592ecb2173593736c. TLDR: the test intended to protect keeping the old image
until the new one is safely recorded would also pass if deletion happened before that recording, leaving this
crash-safety promise without the claimed regression proof. The named test
a_rebuild_deletes_the_old_snapshot_only_after_recording_the_new_one reads state only after f.build completes (:531-536).
Its intermediate order assertions compare get_snapshot(new) versus delete(old) at :539-540 and terminate(second builder)
versus delete(old) at :543-545. Neither event observes the database write. A wrong implementation that completed the
provider snapshot, terminated the builder, deleted the old snapshot, and only then saved the new state would satisfy all
these comparisons and the final-state assertions. MemoryApi::call/delete_snapshot has no store observation or
state-write event; it only logs provider calls and changes the in-memory model. The nearby failed-step test exercises a
script error before new snapshot creation, and builder-recovery test :1330-1358 observes builder-ID attribution, not
current-snapshot commit before superseded deletion. Governing SPEC.md:318-319 promises rebuild replacement only once new
snapshot is recorded; the supplied test-authoring checklist requires an observable distinguishing competing mechanisms.
Correct implementation does satisfy the order (build.rs:806-824 at this original revision, corresponding ca28 :896-924);
this is a verified test-quality finding, not a claim the current engine deletes early. Fix direction: have the fake
old-snapshot deletion boundary query/assert persisted current+retired state, or pause at that boundary and read store,
then use an injected recording failure to prove old snapshot remains. Suggested bucket other (maintainer test evidence
only); cannot belong to highest based on this test gap alone. Confidence confirmed by source, no mutation experiment or
runtime tests. proposed_drop false; possible_cover unknown, parent handles. Test unchanged except the later insertion of
ensure_snapshot test, so original function shifts to ca28 :580-600 and remains through pinned tip per scoped git log.
This is new discovery d2, separate from d1 and the known lifecycle races.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g070/d2`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
