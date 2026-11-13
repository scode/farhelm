# Whole-host Delete does not record sealed deletion failures

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Deleting a sealed sandbox or container through Hosts can leave its persistent error showing the previous failure instead of the failure just received. If the seal was pending, a failed whole-host Delete remains pending and can be retried automatically instead of showing its error and waiting for Retry.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g027/l4`.

### g027/l4

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 8 (finding, open):

Reviewed commit: 222bf6253bcd168db04bcbb1df20180c88306998

TLDR: Deleting a sealed sandbox or container through Hosts can leave its persistent error showing the previous failure instead of the failure just received. If the seal was pending, a failed whole-host Delete remains pending and can be retried automatically instead of showing its error and waiting for Retry.

Details: remove_host:1375-1383 -> remove_host_for:1393-1403 and remove_host_checked:1420-1436 call forget_host_locked directly. Its provider-error branch:1537-1545 releases the actor hold and returns without writing sealed_outcome. The post-destruction local-cleanup failure branch:1555-1565 also releases and returns without an outcome. Only successful forgetting clears the seal at 1575. The seal wrapper in sessions/sandbox.rs:290-304 records outcomes, but these ordinary-host-removal callers bypass it. Consequently an existing Failed(old_reason) remains unchanged after another ordinary Delete fails with new_reason, while an existing Pending remains eligible for a future finisher scan.

Admission checked in synthesis: the GUI's on_remove_start (crates/farhelm-ui/src/hosts.rs:2540-2566) checks page operation/provisioning busy state, not Deleting/Pending/Failed. Its remove answer binds the rendered ConfirmToken, extracts the captured host ID and calls remove_host at 2525; menu Remove:5321-5353 is disabled only by busy. Row busy:3350-3362 derives from page mutations/provisioning/page lock, not the seal state. Server remove_host_for takes a nonwaiting provisioning lock and never refuses a sealed host. Thus a Failed host is admitted after the earlier attempt finishes. Pending is admitted if the nonwaiting lock is free (for example, deletion confirmed while a seal is first being noted); a running destroy holds the lock and is refused as busy. Separate Retry (UI hosts.rs:2363-2371) correctly calls retry_environment_delete and does not fix ordinary Delete.

Projection/release checked: manager snapshots:2165-2200 and status:2210-2235 always substitute sealed_state for the actor state, and status withholds its client. release_hold clears the actor hold, advances its nudge and permits reconnect, but never changes the seal record. note_sealed preserves an existing Failed/Pending entry. The old failure therefore remains after the API's current error/dialog is dismissed; reconnect cannot repair it. Contract: SPEC_impl.md:6642-6646 says the record holds "pending or with its last destruction's failure" and "the failure and its Retry cannot be lost".

Later in-range check: git log and complete path diff from this commit through 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db show no production changes in hosts.rs, manager.rs, sessions/sandbox.rs or UI hosts.rs; only f560058da19e02716e6f943883a9508b447aff4e removes two manager-test fixture fields. This finding remains at the endpoint. Suggested fix: record attempt/outcome for every already-sealed removal, without inserting unsealed hosts into the seal map.

Cross-input/provenance: parent supplied g028/l3, a distinct stale Pending Vec bug in the finisher. That path starts from correctly recorded direct-attempt failure, but uses captured eligibility without rechecking after taking the lock. l4 instead fails to write an outcome on ordinary host removal and needs no stale captured Vec: a later fresh scan still sees Pending, or Failed retains old_reason. Fixing either cause alone does not fix the other. Preserve both IDs and owners.

Suggested bucket: other (stale diagnostics and retry policy for already-authorized destruction). Could-be-highest: no; the permanent seal already authorizes destruction of the recorded environment, and this establishes no work loss beyond that consent. Confidence: confirmed by source only; no execution. proposed_drop: false. possible_cover: none; related g028/l3 supplied by parent is not a duplicate cause. Caveats: unsealed whole-host failures do not require seal bookkeeping; Pending requires its seal be known before removal completes, while the simpler Failed case needs no such race. Revision: synthesis confirms admission, projection, release ordering and no later in-range fix; original consequences and priority retained.

Returned proposal (claims-015, event 8): **retain-verified**, suggested bucket **other**.

Ordinary Hosts Delete bypasses the seal outcome wrapper, so a failed attempt preserves an obsolete failure or Pending eligibility. This is persistent bookkeeping/retry-policy failure on destruction the user already authorized.

Original stronger consequence: Unchanged throughout: obsolete persistent error and unintended retry of already-authorized sealed-environment destruction. No wrong resource or work loss beyond consent is established.

Unresolved premise or limitation: A provider/local-cleanup failure must occur. The Pending variant additionally needs a known seal while ordinary removal obtains the free provisioning lock; the Failed(old_reason) variant needs no such race.

Proposed basis: Record both failure branches and preserve the simpler Failed case. Keep separate from stale finisher-snapshot eligibility.

Existing parent disposition (event 8): verified. Parent explicitly accepted named retain-verified proposal from claims-015 after inspecting it. Ordinary Hosts Delete bypasses the seal outcome wrapper, so a failed attempt preserves an obsolete failure or Pending eligibility. This is persistent bookkeeping/retry-policy failure on destruction the user already authorized. Record both failure branches and preserve the simpler Failed case. Keep separate from stale finisher-snapshot eligibility. Checked source/results: ["222bf6253bcd168db04bcbb1df20180c88306998:hosts.rs:1375-1402,1422-1437 calls forget_host_locked directly. Its provider-error branch :1537-1545 and post-destruction cleanup-error branch :1555-1564 return without sealed_outcome; success alone clears it at :1575.","sessions/sandbox.rs:290-304 supplies sealed_attempt and sealed_outcome for the seal path. Ordinary removal does not pass through it.","UI hosts.rs:2502-2535 sends ordinary remove_host for the captured host; :2540-2568 checks operation/provisioning busy state, not seal state. Row busy :3355-3361 and Remove handler :5321-5353 allow another attempt on a Failed sealed host after prior work finishes.","manager.rs:2606-2651 preserves an existing seal on note_sealed and changes outcome only through sealed_outcome. snapshots/status :2166-2228 project the seal over connection state and withhold clients, so reconnect does not refresh the saved error.","Path diff from this commit to 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db shows only two manager test-fixture field removals, no production repair. No runtime failure was observed."] Current coverage/results: ["SPEC.md Sandbox sessions requires visible failure and Retry; current SPEC_impl.md:6148-6157 requires the last destruction failure and Retry-only failed state. The source violates these rather than implementing an accepted limitation.","TODO Planned is empty; BUGS.md has no matching exception. Active TRIAGE_OUTCOMES.md and INDEX.md searched for sealed deletion errors/Pending retry: no exact item. plans/queue/environment-agent-operations.md:524-530 requires reason/Retry but does not separately schedule this bypass repair.","FILTER.md's rare-diagnostic filter does not wholly match: Pending can remain automatically retryable, and the retained failure survives reconnect. Already-Failed ordinary Delete also needs no narrow timing race.","Supplied g028/l3 concerns stale captured finisher eligibility after a correctly recorded failure. It is a different cause; no alias is asserted without its assigned record."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Unchanged throughout: obsolete persistent error and unintended retry of already-authorized sealed-environment destruction. No wrong resource or work loss beyond consent is established."; "A provider/local-cleanup failure must occur. The Pending variant additionally needs a known seal while ordinary removal obtains the free provisioning lock; the Failed(old_reason) variant needs no such race.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g027 record](records/review-through-00de0b6c/groups/g027.json). Record ID: `l4`.

