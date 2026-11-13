# Direct grid input call preserves the prior delivery contract

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: replacing the async backend wrapper with a direct grid-input call does not establish newly premature hook warnings or weaker ownership checks.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g124/c3`.

### g124/c3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 17 (candidate, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: replacing the async backend wrapper with a direct grid-input call does not establish newly premature hook warnings or weaker ownership checks. Details: parent terminals.rs:1976-1984 already dispatches its Host branch directly to host.send_input. New connection.rs:507-513 keeps the same attachments lock and channel-plus-connection owner check, and :532-535 keeps Agent plus Ok(true) gating. HostAttach::send_input reports whether encoded input queued for its pump submits a line; it does not await runtime delivery. Discovery attribution check across the whole pinned range: exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a:connection.rs:528-542 already contains the 'whole frame must have been confirmed' and Enter-arrival wording; base terminals.rs:1986-1993 already returns host.send_input for Host, and base grid_stream.rs:1058-1074 already documents queue acceptance as delivery for this mark. Thus both the actual queued-input behavior and the imprecise whole-frame confirmation explanation predate the entire range, not just this assigned commit. The changed comment now points to HostAttach by its correct remaining name, without adding a new delivery invariant. Suggested bucket: other; no established in-scope regression. Confidence: confirmed source comparison, including exclusive base, no execution. proposed_drop: source refutation for runtime changes and pre-range attribution for the retained wording, not merely current production correctness refuting a distinct documentation issue. Could-be-highest: no for hook-warning timing; the security-sensitive ownership check remains intact. possible_cover: none; prohibited coverage sources not searched. Caveat: this does not dispose of l1's independently changed natural-detach input lifetime. This revision adds the required full-range attribution check and preserves the original direct-call/ownership basis.

Returned proposal (claims-079, event 17): **out-of-range**, suggested bucket **other**.

The direct call keeps the host branch's existing queue-acceptance semantics and both ownership checks. The imprecise whole-frame-confirmed explanation also exists at the exclusive base. Thus the retained wording issue predates this range, and the claimed behavioral weakening is source-refuted.

Original stronger consequence: The initial concern was newly premature hook warnings or weakened ownership after removing the async wrapper. The latest record preserves the guard refutation and adds full-range attribution; this does not dispose of g124/l1.

Unresolved premise or limitation: No in-range change to grid delivery semantics, hook timing or the ownership guard is shown.

Proposed basis: Pre-range attribution for the retained imprecise comment, and unchanged source behavior for the direct-call/ownership suspicion. Marked for independent checking conservatively because the rejected concern includes terminal-input ownership.

Existing parent disposition (event 17): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g124 record](records/review-through-00de0b6c/groups/g124.json). Record ID: `c3`.

