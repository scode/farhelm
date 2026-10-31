# Old executor cleanup can reset a new holder's bookmarks

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

An old executor's prescribed full-close cleanup can reset plan bookmarks already being used by the new holder.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The recovery/full-close instructions do not consistently bind shared bookmark cleanup to the old claim after a blocked
plan has been answered and reclaimed. The conditional ownership mismatch is source-established; destruction of commits,
stale publication and user-work loss were not established. Operational-artifact coverage remains ambiguous. No plan or
bookmark cleanup was executed.

Source locations:
`plans/.agents/executing.md:45-48,95-110; plans/.agents/close-reset.md:36-40; plans/.agents/outside-files.md:9-14 at d6645f771bee8ed03a66486ef8755834937d759d`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d. Newly found maintained handoff/cleanup defect in the contracts
used by assigned block/answer/claim/delivery transitions; preexisting, not claimed as introduced in this range. TLDR:
when executors use the supported sibling jj workspaces, an old executor resuming after it blocked a plan can rewind or
delete the new executor's branch markers while cleaning up its previous run. Established prescribed sequence: executor A
successfully blocks, then crashes before step 12. block clears its claim immediately (plans-queue.py:992-999). The
monitor answers an ordinary block (landing.md:64-66,150-169), and B claims and resumes it
(plans-queue.py:976-982,1025-1034). B fetches/rebases the existing stack in its own sibling workspace and moves a
plan/<slug>/* bookmark to its new commit (executing.md:67-94). The shared-workspace arrangement explicitly shares
bookmark state (outside-files.md:9-14); different plan slugs avoid collisions only before a handoff. When A resumes,
executing.md:45-48 expressly says that finding its block/delivery history means finish the full close 'whatever the line
says now'. That close resets every local plan/<slug>/* bookmark to @origin with --allow-backwards, or deletes a
never-pushed bookmark (close-reset.md:36-40). There is no new-holder exclusion. Therefore if B has rebased/amended its
stack but not pushed it, A puts its bookmark back on the older remote commit; if B has created an unpushed additional
plan bookmark, A deletes that name. This can happen after an arbitrarily long interruption, without overlapping jj
commands or a person winning a narrow timing window. The pre-push queue check does not prevent it: B still correctly
holds the claim. The narrow released-claim reset (close-reset.md:52-58) avoids bookmark mutation, but is not selected
for A's successful block/delivery recovery. Unclaim recovery already distinguishes another holder at executing.md:41-44,
demonstrating the missing distinction in the next branch. Commit destruction and user-data loss are NOT established: the
abandon revset excludes work built on by another workspace (close-reset.md:42-46), and B's working copy can retain the
new commit after bookmark displacement. Current jjstack consumer contract (local skill source 225-254) makes bookmarks
its PR-head identity and refuses unbookmarked intermediate commits, so publication may refuse or require reconstruction;
do not claim silently published stale code without an established consumer trace. Confidence: confirmed conditional
command/ownership mismatch under the explicitly supported shared-bookmark contract; actual runtime incidence and exact
next jjstack refusal unmeasured. Suggested bucket other for maintainer branch-state disruption, not highest; no security
or catastrophic user-work-loss route established. Suggested correction: retiring-owner cleanup must exclude state
acquired by a subsequent holder, using an ownership-aware/narrow close or a handoff boundary that completes cleanup
before reuse. No VCS command, experiment, test or build ran. Coverage searches across SPEC.md, SPEC_impl.md, TODO.md,
BUGS.md, queue/index, ledger and planning callers found no exact bookmark-handoff duplicate; Planned and unlanded-plan
queue are empty. plan-pruning-can-interrupt-shared-cargo-build.md:13-27 and its ledger heading at 20353-20366 concern
target artifacts/Cargo locking, not branch markers. possible_cover: operational-artifact exclusion
TRIAGE_OUTCOMES.md:20479-20495; whether it extends to maintained cleanup commands acting on a new holder's shared
resources is ambiguous, so retained. No inference about historical private approvals. FILTER rare-glitch coverage does
not match arbitrary delayed recovery resetting another holder's persistent branch state. proposed_drop: none.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g001/f4`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
