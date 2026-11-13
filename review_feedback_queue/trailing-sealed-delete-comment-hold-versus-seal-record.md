# Deletion test comment wrongly forbids releasing the failed host's connection hold

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: A new test tells maintainers that releasing a failed deletion's host hold would lose the error and Retry control, although the implementation intentionally releases that hold and preserves the error separately. The explanation misstates the lifecycle invariant the test protects; no current runtime malfunction is claimed.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g021/f6`.

### g021/f6

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 15 (finding, open):

Reviewed commit: 222bf6253bcd168db04bcbb1df20180c88306998

TLDR: A new test tells maintainers that releasing a failed deletion's host hold would lose the error and Retry control, although the implementation intentionally releases that hold and preserves the error separately. The explanation misstates the lifecycle invariant the test protects; no current runtime malfunction is claimed.

Details: a_listed_seal_is_finished_by_the_helm_and_a_failure_waits_for_retry at agent_requests.rs:18083-18085 says `its own failure must not release the host: that would reconnect to the sealed Farhelm and lose the deletion Retry.` The preceding block (:18068-18081) deliberately reconnects and asserts the failure remains shown with no routable client. Production hosts::forget_host_locked explicitly calls state.manager.release_hold(host, true) on a provider error (hosts.rs:1537-1544). sessions/sandbox.rs:274-277 states the actual lifecycle: `A failure releases the host's connection hold, so its actor reconnects to the sealed supervisor, but the failure is recorded in the manager's seal record`, which keeps the reason and no client. manager.rs:2610-2620 preserves existing Failed on repeated seal reports; sealed_state/status projection keeps the host visibly Deleting and suppresses its usable client independently of the actor connection. The new comment wrongly makes reconnection itself the cause of lost Retry. The invariant is preserving seal state/display and routing suppression through reconnection, not retaining the connection hold.

Contract: SPEC_impl.md:6643-6646 requires the recorded failure and no client `whatever its connection does meanwhile (held, released, reconnected, retired), so the failure and its Retry cannot be lost`. Reword the test comment around that invariant; its current causal explanation could misdirect lifecycle maintenance toward retaining a hold the intended mechanism releases. The executable assertion at agent_requests.rs:18091-18097 only requires Deleting with a failure, which is compatible with the intended release/reconnect. Production correctness does not refute this bounded documentation defect.

Attribution/range: the entire comment is new in the assigned 222bf625 patch. The complete later path history has only f560058da19e02716e6f943883a9508b447aff4e removing unrelated protocol fixture fields; full base-to-tip diff retains this comment unchanged. Exact numbered anchor :18083-18085 checked. Suggested bucket: other (internal documentation correctness). Could-be-highest: false; no established security or user-work-loss route. Confidence: confirmed source/contract discrepancy. proposed_drop: false. possible_cover: supplied g027/l4 reports a separate production omission of updated seal-failure recording on whole-host Delete, not an error in releasing the hold; parent owns deduplication. Caveats: this record does not re-report that production issue or claim the test's current executable behavior is wrong. All readily checkable premises settled. No runtime tests, builds or reproductions performed.

Returned proposal (claims-011, event 15): **retain-verified**, suggested bucket **other**.

The test comment says releasing the failed host would reconnect and lose Retry, while this implementation deliberately releases the connection hold and preserves the deletion failure in a separate seal record. Its causal explanation misstates the lifecycle invariant. The executable assertion remains compatible with the intended release and reconnect.

Original stronger consequence: The misleading explanation could direct future maintenance toward retaining a connection hold that the intended lifecycle releases. Both history entries limit the claim to documentation correctness and preserve that the current executable assertion is compatible with reconnect.

Unresolved premise or limitation: None for the source/comment discrepancy. Any later maintenance mistake caused by following the comment is speculative; no security, user-work loss or current runtime defect is established by this record.

Proposed basis: Retain as other, separately from production failure-recording defects. Source comments in the newly added test are in range, and production correctness does not refute their incorrect explanation.

Existing parent disposition (event 15): verified. Parent primary checks and complete claims-011: new test comment18083-18085 claims reconnect on releasing the failed host loses Retry, while intended implementation hosts1537-1544 releases the hold and manager seal record preserves failure across reconnect; SPEC_impl6643-6646 expressly says held/released/reconnected/retired all preserve it. Test assertion is correct but explanation teaches wrong lifecycle invariant. No current BUGS/Planned/unlanded-plan/queue/active-ledger cover or full filter match. Documentation-only consequence, no destructive authority inferred.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g021 record](records/review-through-00de0b6c/groups/g021.json). Record ID: `f6`.

