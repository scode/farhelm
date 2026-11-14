# Deletion ownership refutation remains separate from warning repair

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Deletion ownership refutation remains separate from warning repair

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g059/c3`.

### g059/c3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 16 (candidate, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b, with later stability checked through
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. Original TLDR/consequence considered: cross-provider deletion could forget
resource ownership or lose its exclusion on browser disconnect, permitting destructive build/cleanup races.
Could-be-highest: yes for the original possible destruction/ownership-loss consequence; no drop/lowering is accepted by
this reviewer. Confidence: confirmed source refutation of the reviewed admission/lifetime paths; no execution.
proposed_drop: source boundaries already recorded under c3 remain intact, rather than relying on l2's later wording
repair. tensorlake/recipes.rs:586 owns the deletion body through run_owned, lib.rs:2721-2728 owns its tokio task,
begin_delete (:1769-1782) holds shared admission and production construction shares its registry (lib.rs:2465-2472).
Podman start/use/cleanup checks :679-763 exclude work under deletion; Tensorlake start and snapshot-use checks :930-933
and :1957-1971 do likewise. clean_tensorlake :1787-1876 and clean_images build.rs:764-799 wait active work and retain
unsettled ownership; the final store transaction recipes.rs:393-430 rechecks hosts and aggregate needs_cleanup. New
synthesis result: git diff from this revision to the pinned tip is empty for tensorlake/recipes.rs, tensorlake/build.rs
and environment/recipes.rs; environment/mod.rs changes only a cfg(test) setter and leaves production cleanup dispatch
unchanged. Podman build.rs is also unchanged through the tip (l1 synthesis check). Thus the warning repair neither fixes
nor weakens the independently checked ownership contract. possible_cover: agrees narrowly with g060/c3's aggregate-state
check, not a new queue-coverage assertion. Contract: SPEC_impl.md 'Shared recipes and prepared artifacts' and 'Who owns
an accepted action'. Suggested bucket: no active finding proposed, with all original potentially highest consequences
retained for the parent's independent check. Caveat: this is admission/lifetime/retention analysis, not certification of
every underlying provider command. Late-interface scope clarification: the verified default-pruning route canonicalized
as g061/l8 (alias g058/c2) is not refuted by this admission/lifetime argument. c3 establishes that the handler owns its
work, takes shared deletion admission and retains recorded ownership until explicit cleanup confirms; it does not
establish that the external removal command affects only its explicit target. The final aggregate guard runs after
provider cleanup and cannot prevent the implicit user-base deletion now corrected under l2. Preserve the original
potential ownership-loss consequence and source-refutation proposal only for the disconnect/admission/bookkeeping
mechanism stated here. No production-correctness certification or accepted drop is implied, and the separate historical
warning-repair proposal cannot authorize base pruning. No new source lead or execution.

Returned proposal (claims-037, event 10): **refuted**, suggested bucket **other**.

The checked HTTP disconnect cannot drop deletion work or its shared admission: server-owned work retains the guard
across both provider cleanups and final deletion. This narrow conclusion does not certify provider image semantics or
erase existing same-name/detached-write races.

Original stronger consequence: Cross-provider deletion could forget resource ownership or lose exclusion when the
browser disconnects, permitting destructive build/cleanup races or ownership loss. Neither the warning repair nor this
narrow admission proof certifies all provider cleanup commands.

Unresolved premise or limitation: No checked path makes a browser disconnect release deletion admission early. Internal
intentional cancellation, detached writes, row reincarnation and implicit image pruning are separate mechanisms and
remain outside this refutation.

Proposed basis: Source-refute only loss of the checked server-owned deletion admission/lifetime on disconnect; send that
destructive hypothesis to independent drop review.

Existing parent disposition (event 10): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: source-refuted-browser-disconnect-loses-recipe-deletion-admission. Full037 run_owned keeps
deletion task/shared guard across both provider cleanups and final immediate aggregate needs_cleanup transaction; both
providers use the same registry and reject new job/use admission. Browser waiter loss does not release this owned work.
Refute only that disconnect lifetime route, not detached writes, same-name reincarnation, registration races,
intentional internal cancellation or implicit image pruning, all explicitly separate. Preserve destructive
ownership/exclusion hypothesis for fresh check.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g059 record](records/review-through-00de0b6c/groups/g059.json). Record ID: `c3`.
