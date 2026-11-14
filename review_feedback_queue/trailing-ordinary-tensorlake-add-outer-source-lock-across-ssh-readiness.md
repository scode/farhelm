# Ordinary Add holds recipe source exclusion while waiting for SSH

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Ordinary Add holds recipe source exclusion while waiting for SSH

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g045/l1`.

### g045/l1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 14 (finding, open):

# Ordinary Add retains the recipe lock after Running

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b

## TLDR

One ordinary Tensorlake host whose sandbox is Running but whose SSH gateway is unavailable delays other launches and
completion of builds/preparations using its recipe. Billed builders may remain alive while publication waits. This is
extra serialization across transport readiness, not a demonstrated permanent deadlock.

## Details

The caller acquires lock_snapshot_use at tensorlake_add.rs:1352, retains source_guard at 1360, awaits
create_host_sandbox at 1378-1380 and drops the guard only at 1385. The helper passes None as create_recorded's guard and
Some(self.backend) (1599-1616). tensorlake/hosts.rs:471-484 observes Running and drops only that None; :485-497 then
awaits wait_tensorlake_ssh while the real caller-owned guard remains held. Previously the explicit readiness wait
followed the outer drop. The ordinary production task is independently spawned (tensorlake_add.rs:903-911), so browser
request loss does not cancel this wait or release the lock. Its error propagates with ? and drops source_guard on
unwind; normal success drops it at 1385. Host removal cannot cancel this active operation: hosts.rs:1385-1393 and
:1435-1445 first try its held provisioning lock and return busy. delete_registered_host aborts only once that separate
guard permits removal (service.rs:2125-2155); abort_run at 2195-2203 is test-only. Whole task/runtime cancellation drops
the async future's owned guard; it is not a leaked mutex guard. No recursive lock acquisition or wait for a peer that
itself requires the source lock was found in the gateway wait, so there is no established permanent cycle.

The production wait_sandbox_ssh uses a 60-second retry deadline, 15-second individual run_shell calls and one-second
retry pacing (provisioning/tensorlake_backend.rs:58-89). run_shell awaits capture_child (backend.rs:911-939); timeout
kills/reaps the child group and drains pipes within CAPTURE_PIPE_GRACE (:1979-2019,2251-2278). The 60 seconds is a retry
budget, not a strict 60-second wall-clock cap: the last attempt and cleanup can extend it. This synthesis therefore
replaces any implication of an exact hard 60/75-second total with nominally about a minute plus bounded command/pipe
cleanup and normal OS reap/scheduling assumptions. The source shows no permanent recipe-lock deadlock caused by this
change.

Competing session create takes the same source mutex (tensorlake/hosts.rs:227); built/prepared publication also takes it
(tensorlake/build.rs:1245,2443-2445). Contract: SPEC_impl.md, Tensorlake prepared snapshots: "One per-recipe async mutex
excludes source selection/create through the first observed Running state from snapshot publication and deletion.
Installation and supervisor attachment run outside that exclusion." Transfer the real outer guard into create_recorded,
or retain ordinary Add's readiness wait after the outer drop. g051/c3's preparation ownership and c6's bounded readiness
checks neither release nor refute this outer guard.

Suggested bucket: other (bounded launch/build delay and unnecessary billed-resource lifetime). Could-be-highest: no
established security or work-loss route. Confidence: confirmed source/contract analysis, not observed execution.
proposed_drop: false. possible_cover: none. Caveats: new ordinary prepared Add only; recovery with no selected source
and provider-based session launches lack this outer-guard mismatch. Post-Running store-read failure also exits via ? and
drops the guard. No reproduction or runtime tests performed. Only the unrelated sealed-host patch subsequently changes
this file in the assigned range.

Returned proposal (claims-028, event 14): **retain-verified**, suggested bucket **other**.

The ordinary prepared Add path retains the real recipe source guard across the provider's SSH readiness wait after
Running. Concurrent same-recipe creates and snapshot publication wait unnecessarily; the provider only drops its own
absent guard.

Original stronger consequence: Same-recipe launches and build/preparation publication are delayed while a Running host's
SSH gateway is unavailable. Earlier numeric bounds were refined to a retry budget plus cleanup overhead; no proven
permanent deadlock was withdrawn.

Unresolved premise or limitation: Actual latency, concurrency frequency and additional cost are unmeasured. They are not
needed for the bounded extra-serialization finding; no security, work-loss or permanent-cycle route is established.

Proposed basis: Retain the precise source/contract defect at other priority. Transfer the actual outer guard to the
shared helper, or restore readiness waiting after dropping it. Do not claim a strict wall-clock cap.

Existing parent disposition (event 14): verified. Parent explicitly accepted named retain-verified proposal from
claims-028 after inspecting it. The ordinary prepared Add path retains the real recipe source guard across the
provider's SSH readiness wait after Running. Concurrent same-recipe creates and snapshot publication wait unnecessarily;
the provider only drops its own absent guard. Retain the precise source/contract defect at other priority. Transfer the
actual outer guard to the shared helper, or restore readiness waiting after dropping it. Do not claim a strict
wall-clock cap. Checked source/results:
["56659839785691f76da3af5d8f51b511cff4ba8b:crates/farhelm-helm/src/provisioning/tensorlake_add.rs:1352-1385 selects
under lock_snapshot_use, stores source_guard, awaits create_host_sandbox and only then drops it.","Same revision,
:1590-1617 calls create_recorded with guard=None and backend=Some. tensorlake/hosts.rs:471-497 waits for Running, drops
that None, then awaits wait_tensorlake_ssh before returning. The caller's guard remains owned across the
wait.","tensorlake/hosts.rs:227 takes the same lock for another prepared session create; tensorlake/build.rs:1245 and
:2443-2445 takes its snapshot_mutex for built/prepared publication. These independently awaited operations can queue
behind the outer guard.","tensorlake_backend.rs:58-89 uses a 60-second retry budget and 15-second attempts with retry
pacing. This is nominally about a minute plus final command/cleanup/scheduling overhead, not a proven strict
60/75-second wall-clock cap or permanent deadlock.","Complete tensorlake_add.rs diff from
56659839785691f76da3af5d8f51b511cff4ba8b to 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db adds only the unrelated seal
refusal. No runtime latency or billed cost measured."] Current coverage/results: ["SPEC_impl.md:6560-6564 defines the
first observed Running state as the exclusion boundary and explicitly puts installation/attachment outside it. Adding
Tensorlake hosts separately acknowledges Running can precede gateway readiness (:6314-6319); it does not extend source
exclusion across that wait.","TODO.md Planned and plans/queue/INDEX.md empty; BUGS.md's escaped SSH transfer hang
differs from this normally bounded gateway wait.","Full INDEX plus targeted queue/active ledger searches found no exact
outer-guard readiness item. snapshot-cleanup-races-new-host-registration.md and snapshot-state-races-build-admission.md
read in full: they concern source admission/retirement gaps, not unnecessary guard retention after Running, and are not
exact cover or same-root aliases.","No FILTER.md match: the established consequence includes delaying independent
same-recipe launches/publications and extending a builder's billed lifetime, rather than only a local diagnostic or
safely retried operation."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved:
"Same-recipe launches and build/preparation publication are delayed while a Running host's SSH gateway is unavailable.
Earlier numeric bounds were refined to a retry budget plus cleanup overhead; no proven permanent deadlock was
withdrawn."; "Actual latency, concurrency frequency and additional cost are unmeasured. They are not needed for the
bounded extra-serialization finding; no security, work-loss or permanent-cycle route is established.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g045 record](records/review-through-00de0b6c/groups/g045.json). Record ID: `l1`.
