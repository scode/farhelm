# Suspended view documentation retains the known Tensorlake-only vocabulary

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: While checking newly projected states, noted that HostStateView::Suspended docs still describe only a Tensorlake host even though Podman paused containers now use it. This is the documentation side of the already recorded shared-state provider-vocabulary generalization (c4), rather than a newly discovered wrong-runtime-dispatch route. Source: hosts.rs:250-258 starts 'A Tensorlake host whose sandbox Tensorlake reports suspended' and attributes resume_failed to Tensorlake; From at :415-423 copies shared HostState::Suspended for both providers. The manager's changed type docs at manager.rs:599 onward correctly describe the shared provider state. The literal documentation discrepancy is real; production dispatch correctness does not refute it. Suggested bucket: other. Confidence: confirmed source text, no execution. Could-be-highest: false; no security or loss-of-work route is claimed. proposed_drop: proposal not to create an additional standalone finding for this second Tensorlake-only explanation of the same newly shared Suspended state; preserve it here for caller grouping with c4, not as specification acceptance, filtering or an accepted parent drop. possible_cover: related own c4 only, no coverage search. Caveat: c4's concrete user-visible error and this maintainer-facing enum documentation are different artifacts; the caller may retain the documentation separately if that grouping is too broad. No open investigation is hidden by this proposal.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g024/c5`.

### g024/c5

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 20 (candidate, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b. TLDR: While checking newly projected states, noted that HostStateView::Suspended docs still describe only a Tensorlake host even though Podman paused containers now use it. This is the documentation side of the already recorded shared-state provider-vocabulary generalization (c4), rather than a newly discovered wrong-runtime-dispatch route. Source: hosts.rs:250-258 starts 'A Tensorlake host whose sandbox Tensorlake reports suspended' and attributes resume_failed to Tensorlake; From at :415-423 copies shared HostState::Suspended for both providers. The manager's changed type docs at manager.rs:599 onward correctly describe the shared provider state. The literal documentation discrepancy is real; production dispatch correctness does not refute it. Suggested bucket: other. Confidence: confirmed source text, no execution. Could-be-highest: false; no security or loss-of-work route is claimed. proposed_drop: proposal not to create an additional standalone finding for this second Tensorlake-only explanation of the same newly shared Suspended state; preserve it here for caller grouping with c4, not as specification acceptance, filtering or an accepted parent drop. possible_cover: related own c4 only, no coverage search. Caveat: c4's concrete user-visible error and this maintainer-facing enum documentation are different artifacts; the caller may retain the documentation separately if that grouping is too broad. No open investigation is hidden by this proposal.

Returned proposal (claims-013, event 20): **retain-verified**, suggested bucket **other**.

The enum documentation inaccurately limits the now-shared Suspended state to Tensorlake. It is related to c4 but names a different artifact; correcting the refusal alone would leave this explanation false.

Original stronger consequence: The author proposed omitting a standalone item by grouping this documentation with c4, explicitly preserving its real discrepancy and absence of a wrong-runtime-dispatch claim.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Preserve as an explicit documentation extension if the parent groups provider-vocabulary omissions into c4; otherwise record separately. Do not silently mark exact duplicate merely because both mention Tensorlake.

Existing parent disposition (event 20): verified. Parent explicitly accepted named retain-verified proposal from claims-013 after inspecting it. The enum documentation inaccurately limits the now-shared Suspended state to Tensorlake. It is related to c4 but names a different artifact; correcting the refusal alone would leave this explanation false. Preserve as an explicit documentation extension if the parent groups provider-vocabulary omissions into c4; otherwise record separately. Do not silently mark exact duplicate merely because both mention Tensorlake. Checked source/results: ["56659839785691f76da3af5d8f51b511cff4ba8b:crates/farhelm-helm/src/hosts.rs:250-258 describes Tensorlake reporting suspension and supplying resume_failed; :415-423 copies the shared manager Suspended state.","Same revision environment/podman.rs:1258-1262 sends paused Podman resources through that state. Stored provider dispatch remains correct; no runtime wrong-provider operation is inferred."] Current coverage/results: ["SPEC.md Container sessions:4850-4865 and SPEC_impl.md Disposable environments:5940-5981 establish shared lifecycle state and provider-owned wording, rather than accepting a Tensorlake-only view contract.","TODO.md Planned is empty. Searched BUGS.md, plans/queue, INDEX.md and active TRIAGE_OUTCOMES.md for shared Suspended vocabulary and Tensorlake-only documentation; no exact cover found.","podman-environments.md:840-847 lists other provider-specific UI wording repairs, not this enum explanation. FILTER.md does not cover a standing internal contract error. g024/c4 is related new evidence for grouping, not exact existing queue coverage."] Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "The author proposed omitting a standalone item by grouping this documentation with c4, explicitly preserving its real discrepancy and absence of a wrong-runtime-dispatch claim."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g024 record](records/review-through-00de0b6c/groups/g024.json). Record ID: `c5`.

