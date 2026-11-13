# Probe deduplication test checks its counter before a duplicate task can run

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: Probe deduplication test checks its counter before a duplicate task can run

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g064/l5`.

### g064/l5

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 14 (finding, open):

# Probe deduplication test checks before a duplicate task can run

Reviewed commit: 04ffdbb35773cef36e73eb798c005ddbeb9a9de6

## TLDR

The background-probe test can pass an implementation that starts two checks for the same host. Its claimed deduplication coverage therefore does not establish the behavior it is supposed to protect. This is a test-oracle defect, not an observed product failure.

## Details

Assigned test refresh_is_background_deduplicated_and_route_bound calls a second refresh at podman_probe.rs:609 and immediately asserts calls==1 at :610. refresh schedules the command chain with tokio::spawn at :201 and returns at :230 without awaiting that task. The test has the default current-thread runtime: farhelm-testtrace-macros/src/lib.rs:55-64 expands the async test through run_async, :247-259 chooses CurrentThread unless explicitly multi_thread, and farhelm-testtrace/src/lib.rs:1131-1142 builds that runtime. Cargo.lock:6278-6280 pins Tokio 1.53.2; its installed primary source src/task/spawn.rs:22-29 explicitly guarantees spawn does not synchronously poll the task and that outstanding tasks are dropped at runtime shutdown. Consequently, if the refresh admission at podman_probe.rs:197-199 regressed to allowing duplicates while retaining its refreshing marker insertion, the second spawned probe would still be unpolled at the immediate counter assertion on this runtime. The first probe is already held at its release.notified await (:567-570); the only notification sent by the test (:613) releases that first waiter. The test then waits only for the shared refreshing set to become empty (:614-620), not for all scheduled refresh tasks. Finishing the first probe removes the host marker in Refreshing::drop (:240-247), even if the duplicate has since reached the held uname. The subsequent success/retarget assertions (:621-637) do not inspect the counter again, join the duplicate, or release its waiter. run_async at farhelm-testtrace/src/lib.rs:1007-1019 drops the runtime after the test body returns and accepts the returned outcome; cancelling a leftover task is not a failed test. Thus a duplicate can remain blocked and be cancelled at successful teardown without failing any assertion.

The current production admission branch is correct; that does not refute this independent test sensitivity problem. Test-authoring contract: assert an observable that distinguishes competing mechanisms, and establish the fixture premise at the action/measurement boundary. The test docstring at :552-554 specifically promises that background refreshes join per host. Fix direction: use an observable admission/task-start boundary for all accepted refreshes, or another deterministic test seam that proves the second request joined before asserting there is only one probe; retain the deliberately held first command. Do not rely on a scheduling sleep.

Range persistence checked by complete assigned-file diff through 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db: this test body is unchanged, now :851-933 (second refresh/counter at :904-905). The later Notify used by runtime/fresh consumers does not add a test join or final counter assertion. Suggested bucket: other/test assurance. Confidence: confirmed by source/contract and pinned runtime semantics; no runtime test, mutation test or reproduction run, and no claim that a false pass was observed in execution. proposed_drop: false. possible_cover: none among the supplied exact interface findings; prohibited coverage sources not searched. Could-be-highest: false; no security or user-work-loss route is alleged. All readily checkable runtime and teardown premises are resolved; original suspected false-pass consequence retained.

Returned proposal (claims-041, event 14): **retain-verified**, suggested bucket **other**.

The regression's immediate counter check cannot observe a duplicate task just spawned by the second refresh. Its later wait witnesses one shared marker clearing, not completion of every scheduled probe. This is an independent test-oracle defect even though current production deduplication is correct.

Original stronger consequence: The test can pass duplicate probes; no product security or user-work-loss route was alleged.

Unresolved premise or limitation: None for the source-level oracle gap. Whether a particular mutation falsely passes in execution was not measured and is not needed to claim the missing distinguishing observation.

Proposed basis: Record as other/test assurance with source-confirmed scheduling and teardown premises, and preserve that this is not an observed production deduplication failure.

Existing parent disposition (event 14): verified. Parent explicitly accepted named retain-verified proposal from claims-041 after inspecting it. The regression's immediate counter check cannot observe a duplicate task just spawned by the second refresh. Its later wait witnesses one shared marker clearing, not completion of every scheduled probe. This is an independent test-oracle defect even though current production deduplication is correct. Record as other/test assurance with source-confirmed scheduling and teardown premises, and preserve that this is not an observed production deduplication failure. Checked source/results: ["04ffdbb35773cef36e73eb798c005ddbeb9a9de6: environment/podman_probe.rs:197-201 admits via a shared set and spawns; no await follows that spawn before refresh returns230. Test609-610 immediately checks the command count after its second refresh. The held uname567-570 has one release613;614-620 waits only for the shared refreshing flag, and621-637 never rechecks count or joins a duplicate.","Same revision, Refreshing::drop240-247 clears that shared host marker when either task finishes. With admission regressed to allow a duplicate while retaining marker insertion, a second probe can remain blocked after the first clears the marker.","Same revision, farhelm-testtrace-macros/src/lib.rs:55-64,247-259 selects CurrentThread by default; farhelm-testtrace/src/lib.rs:1007-1019 drops the runtime after body completion and accepts the body's outcome;1131-1142 builds the current-thread runtime.","Cargo.lock at that revision6278-6280 pins Tokio1.53.2. Installed primary source tokio-1.53.2/src/task/spawn.rs:22-29 guarantees spawn does not synchronously poll and outstanding tasks are dropped at shutdown. Thus the immediate assertion does not see the newly spawned duplicate, whose cancellation need not fail the test.","00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db: podman_probe.rs:851-933 retains the same test, with second refresh/counter904-905 and no final count/join. No mutation test, false pass or runtime reproduction was observed."] Current coverage/results: ["Searched both specifications for background probes/deduplication; SPEC.md Container sessions and SPEC_impl.md Podman places require background capability checks but do not accept an ineffective regression. The test's own552-554 contract promises per-host joining.","TODO Planned is empty. plans/queue searches for probe/deduplication/test names and schema/source names found no planned repair. BUGS.md is unrelated.","INDEX.md includes other ineffective or premature test oracles, but exact probe test/source-name searches of queue files found none for this root. Active TRIAGE_OUTCOMES.md has no matching disposition. FILTER.md's rare product-glitch filters do not cover this persistent test sensitivity defect."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "The test can pass duplicate probes; no product security or user-work-loss route was alleged."; "None for the source-level oracle gap. Whether a particular mutation falsely passes in execution was not measured and is not needed to claim the missing distinguishing observation.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g064 record](records/review-through-00de0b6c/groups/g064.json). Record ID: `l5`.

