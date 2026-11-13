# Restart reply comment promises a removed tab rediscovery path

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: the supervisor's restart-reply documentation tells future maintainers that terminal tabs are re-read after replacement, but this change removes that path. A maintainer using the comment to reason about the freshness and provenance of the reply's tab list is given a false contract. This is a documentation defect, not an established runtime failure or claim that post-restart rediscovery must be restored.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g106/l1`.

### g106/l1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 11 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR: the supervisor's restart-reply documentation tells future maintainers that terminal tabs are re-read after replacement, but this change removes that path. A maintainer using the comment to reason about the freshness and provenance of the reply's tab list is given a false contract. This is a documentation defect, not an established runtime failure or claim that post-restart rediscovery must be restored. Details: core.rs:13747-13749 says, 'Supplied by `relaunch_into_terminal`: pane reuse carries the preflight list, while replacement in a surviving session supplies a post-replacement discovery.' The assigned deletion removes pane reuse, tmux replacement creation, replacement_session_tabs and its post-operation call. New core.rs:13608-13614 obtains session_tabs once before the host start, and :13635-13638 forwards that exact vector into relaunch_into_host. host_restart.rs:393-394 correctly describes the tab strip read beforehand; its confirmation-error publication (:540-549) and successful publication (:569-579) both pass the supplied tabs unchanged. Therefore neither of the stale comment's selection paths remains. Parent proof: parent core.rs:14731-14733 contains the same comment, but parent :14434-14451 still publishes the selected preflight/post-replacement list; its host early-return branch is at :14004-14022. The assigned removal, not merely a wording preference or older unmaintained prose, makes the claimed tmux distinction impossible. Contract: SPEC_impl.md at this revision, Terminal host, Every new terminal in a host and Restarting a host agent, requires every Restart to start a host while keeping the session's tabs running; the new relaunch_into_terminal and host_restart documentation state that same retained-host path. Correct production behavior and nearby accurate prose do not make this false field-level explanation accurate. Fix direction: describe the preflight tab vector carried through the host replacement, which does not add a tmux agent window, instead of implying pane reuse or an additional discovery. Suggested bucket: other (internal documentation correctness). Could-be-highest: no established or suspected security or user-work-loss route from this comment. Confidence: confirmed source/caller comparison, no execution. proposed_drop: false. possible_cover: none in supplied neighbour records; g006/f1 concerns a different stale switch condition in SPEC_impl.md. Material caveat: later-range repair status still being checked; this charter includes the recorded intermediate commit even if a later cleanup fixes the comment. Lead l1 is resolved to this finding, with no unresolved source premise for the reviewed-revision discrepancy. Later-range status resolved: git log -G over the complete reviewed-to-tip core.rs history finds no change to pane reuse carries/post-replacement discovery; the 08399d09 comment-cleanup diff has no matching hunk. Pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:core.rs:13096-13098 retains the stale comment verbatim. The same tip at :3987-3988 correctly documents Relaunched.tabs as the preflight list because Restart keeps every tab open. Thus this is not repaired by later tmux-driver removal or comment cleanup, and accurate adjacent documentation exposes rather than resolves the contradiction. No later-repair premise remains open.

Returned proposal (claims-069, event 11): **retain-verified**, suggested bucket **other**.

The field comment still teaches a removed distinction between pane reuse and post-replacement discovery. Correct production behavior does not repair that false explanation.

Original stronger consequence: A maintainer receives a false contract for the freshness and provenance of restart-reply tabs. No runtime failure, security mistake or loss-of-work route is established.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain as an internal documentation defect introduced within the range and still present at its tip. No filter matches the entire consequence.

Existing parent disposition (event 11): verified. Parent explicitly accepted named retain-verified proposal from claims-069 after inspecting it. The field comment still teaches a removed distinction between pane reuse and post-replacement discovery. Correct production behavior does not repair that false explanation. Retain as an internal documentation defect introduced within the range and still present at its tip. No filter matches the entire consequence. Checked source/results: ["647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513:crates/farhelm-supervisor/src/service/core.rs:13608-13638 reads tabs once before side effects and forwards them to the host restart; :13747-13749 claims two tmux-specific provenance paths.","Same revision:core/host_restart.rs:393-394,540-549,569-579 documents the preflight list and publishes the supplied vector on both branches.","647f0227^:core.rs:14428-14453 actually selects preflight versus post-replacement tabs, so the removal introduces the mismatch. At 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db:core.rs:13096-13098 the false comment remains, while :3987-3988 correctly describes the preflight list."] Current coverage/results: ["SPEC.md:1925 and SPEC_impl.md Terminal host describe tabs surviving Restart; neither accepts false reply documentation.","TODO.md Planned is empty. Exact phrase/rediscovery searches of plans/queue, BUGS.md, INDEX.md, queue files and active TRIAGE_OUTCOMES.md found no matching item. plans/queue/tmux-drop-from-docs.md criterion 3 would have targeted stale comments, but plans/REPORTS.md:32 says it has landed; it is not unlanded coverage and did not fix this line."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "A maintainer receives a false contract for the freshness and provenance of restart-reply tabs. No runtime failure, security mistake or loss-of-work route is established."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g106 record](records/review-through-00de0b6c/groups/g106.json). Record ID: `l1`.

