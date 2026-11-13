# Definitive-Restart documentation overpromises absence of external effects

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: The rewritten failed-Restart documentation says a definitive failure changed nothing outside the supervisor. In fact it can mean a replacement program started and was later cleaned up. Restoring the previous terminal does not establish that the replacement made no external changes, so the explanation gives maintainers a false failure/rollback contract. No production failure or external change was observed during execution.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g105/l3`.

### g105/l3

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. Persistence checked through pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db.

TLDR: The rewritten failed-Restart documentation says a definitive failure changed nothing outside the supervisor. In fact it can mean a replacement program started and was later cleaned up. Restoring the previous terminal does not establish that the replacement made no external changes, so the explanation gives maintainers a false failure/rollback contract. No production failure or external change was observed during execution.

Details: republished_terminal's changed docstring at core.rs:3400-3413 says, "A definitive failure changed nothing outside this process and puts the previous terminal back." Its caller at :13528 restores the prior terminal when the RelaunchFailure is definitive. host_restart.rs:481-489 obtains that failure from unwind_failed_host_relaunch. That helper at :1170-1219 ends a possibly-started host, removes its artifacts, reaps its process tree, and returns RelaunchFailure::definitive when cleanup is confirmed. The helper's own comment at :1197-1200 expressly says, "The start's child may have run for a moment (a host that ended before it could be adopted)." The concrete production route is host_launch.rs:576-623: StartingHost::finish can succeed, then HostTerminal::adopt can fail Gone or Unavailable; the failure returns to that unwind. hosts/spawn.rs:222-224 defines successful readiness as the socket bound and the child started. Thus this is not merely a no-start failure: the replacement's child can run before the definitive cleanup, and neither the sweep nor record restoration rolls back anything it wrote or did outside the terminal. The correct distinction is that no running replacement remains after confirmed cleanup, not that no external effect ever happened.

Contract: SPEC_impl.md at this revision, Terminal host / Restarting a host agent, :2450-2456 describes a failed replacement being ended and swept before definitive restoration. SPEC.md Lifecycle operations :1948-1950 likewise makes confirmed ending of the failed start the condition for returning to the old stopped terminal. Those rules do not promise transaction-like rollback of an already-started program's effects. Fix the comment to describe confirmed absence of a running replacement and restoration of the previous terminal, preserving the ambiguity rule for unconfirmed cleanup.

Attribution: the parent core.rs:3506-3533 said a definitive failure changed nothing "in tmux" and then separately described host restoration. That tmux-specific sentence did not claim host launches or arbitrary external state were untouched. The assigned rewrite broadens it to "nothing outside this process" while making every launch a host. Therefore the false generalized invariant is introduced by this changed documentation, even though the failed-start execution/cleanup behavior already existed. Pinned tip core.rs:3239-3253 retains the same sentence verbatim, so later comment cleanup does not repair it. No earlier-range claim is needed to attribute this specific new wording.

Suggested bucket: other (documentation correctness about a failure contract). Could-be-highest: false; no security or user-work-loss route is alleged from this docstring, and effects of an actually started replacement are not themselves a newly established production defect. Confidence: confirmed source/caller/contract analysis; no runtime tests, builds or reproductions. proposed_drop: false. possible_cover: none among supplied exact neighbour records; those cover distinct stale migration/tab-list comments and failure test oracles. Material caveat: this does not claim Restart should undo program side effects, nor challenge restoring the previous terminal after confirmed cleanup; the defect is teaching that cleanup proves external state was unchanged. Lead l3 is resolved to this finding, with no missing readily checkable source premise.

Returned proposal (claims-068, event 12): **retain-verified**, suggested bucket **other**.

The revised explanation teaches that definitive failure proves no external changes. A child can start before adoption fails and cleanup returns definitive, so that explanation is false. The product need not undo the child's effects; this is the comment's incorrect contract.

Original stronger consequence: Initially suspected an overpromise that definitive Restart failure left all external state unchanged. Source establishes the false maintenance contract, not a requirement to undo program effects, a production rollback defect, or security/user-work loss caused by the comment.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain the newly broadened false comment as other; keep it distinct from the existing unwind-spec omission.

Existing parent disposition (event 12): verified. Parent explicitly accepted named retain-verified proposal from claims-068 after inspecting it. The revised explanation teaches that definitive failure proves no external changes. A child can start before adoption fails and cleanup returns definitive, so that explanation is false. The product need not undo the child's effects; this is the comment's incorrect contract. Retain the newly broadened false comment as other; keep it distinct from the existing unwind-spec omission. Checked source/results: ["647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513:core.rs:3400-3413 says definitive failure changed nothing outside this process. Its immediate parent at 3506-3533 limited the prior statement to tmux and separately described host restoration; the assigned rewrite introduces this generalized external-state claim.","Same revision host_launch.rs:576-623 can finish StartingHost successfully and then fail HostTerminal::adopt as Gone/Unavailable. hosts/spawn.rs:222-224 defines readiness as socket bound and child started. host_restart.rs:481-489 routes Host failure to unwind; 1170-1219 can end the host, remove artifacts, sweep, then return definitive. Its 1197-1200 comment explicitly allows a child that ran briefly.","Restoring the prior terminal at core.rs:13528 neither reverses prior child writes nor proves their absence. Pinned tip core.rs:3239-3253 keeps the same false sentence. No actual external effect or runtime failure was observed."] Current coverage/results: ["Current SPEC.md Lifecycle operations:1944-1950 and SPEC_impl.md Restarting a host agent:2307-2315 describe ending a failed replacement and restoring the old terminal. Neither promises transaction-like rollback of external effects.","TODO.md Planned empty; BUGS.md unrelated; plans/queue searched for definitive/external effects/rollback comments. tmux-drop-from-docs has landed (REPORTS.md:32), and its generic audit is not an unlanded fix.","INDEX.md and matching restart-unwind-omits-tree-sweep-failure.md read in full. That item concerns the specification's exhaustive ambiguous-cleanup condition and host-record reachability, not the new claim that a started replacement made no external changes. This is neither an exact alias nor full coverage. Active TRIAGE_OUTCOMES.md has no matching external-effects decision; archive not read.","FILTER.md has no full match: a persistent false implementation explanation is not merely a rare runtime diagnostic/status glitch."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Initially suspected an overpromise that definitive Restart failure left all external state unchanged. Source establishes the false maintenance contract, not a requirement to undo program effects, a production rollback defect, or security/user-work loss caused by the comment."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g105 record](records/review-through-00de0b6c/groups/g105.json). Record ID: `l3`.

