# failed-relaunch warning policy and restoration route predate the full range

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Failed-restart rollback can accept a successful portable sweep despite an unconfirmed scope. The helper, warning semantics and definitive restoration route all predate the full review range. This does not establish that the retained policy is safe.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g099/l1`.

### g099/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 13 (candidate, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333; attribution checked against exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a and inclusive tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db.

TLDR: Failed-restart rollback can accept a successful portable sweep despite an unconfirmed scope. The helper, warning semantics and definitive restoration route all predate the full review range. This does not establish that the retained policy is safe.

Details: Original concern preserved: a replacement descendant missed by the portable sweep while its scope is unconfirmed could survive definitive rollback and act beside a later agent; user-work damage was not established. Current host_restart.rs:733-782 passes recorded(scope), no root, AgentOnly and Warn. sweep.rs:1527-1538 maps clean sweep plus scope error to Ok. Actual sweep Err instead yields ambiguous failure.

Full-range checks/results: Base host_restart.rs:1177-1226 contains the same helper, Warn at 1215 and definitive return at 1226. Bounded git log -G 'ScopeKillFailure::Warn|unwind_failed_host_relaunch' for that file over the entire pinned range returns no commits. Base sweep.rs:1539-1562 already has the same Warn-maps-Ok / Refuse-maps-Err policy. The full-range sweep diff has no changes to that policy, recorded-unit re-probe or recorded(scope) construction; changes to tab classification and Stop do not alter this unwind's inputs. Base core.rs:13755-13829 already selects definitive versus ambiguous failure, aborts the durable relaunch on definitive, and sets Launching only on ambiguous. Thus the originally suspected restored-scope/descendant issue is not introduced by a prior in-range policy change either. These are source comparisons, not runtime observations.

Cross-group reconciliation: g106/c2's actual-Err -> ambiguous Launching/no-terminal -> guarded Delete route limits consequences for actual Err. It does not refute Warn -> Ok -> definitive restoration. Both branches remain distinct. No new route from the assigned removal was found.

Suggested bucket: other for this attribution disposition; original potential concurrent-program damage remains explicit. Could-be-highest: potentially if surviving concurrent programs damage work; no such damage route established here. Confidence: confirmed pre-range helper/policy/caller presence; independent pre-existing safety concern remains unproven. proposed_drop: pre-existing before exclusive base, superseding the insufficient immediate-parent-only attribution basis in the earlier event. This is a scope proposal, not source refutation or specification acceptance of the residual. possible_cover: none. Caveats: no parent drop or lowering accepted; independent checking must preserve the original potential highest consequence. No queued coverage sources or runtime actions used. No remaining readily checkable attribution premise is open for this ID.

Returned proposal (claims-065, event 13): **out-of-range**, suggested bucket **other**.

The named Warn-to-Ok unwind route exists at the exclusive base. This is an attribution proposal, not a safety refutation.

Original stronger consequence: A replacement descendant missed by a clean portable sweep while its scope is unconfirmed could survive definitive rollback and act beside a later agent, potentially damaging user work. Actual Err-to-ambiguous guards do not refute this Warn-to-Ok route. Earlier immediate-parent-only exclusion was insufficient and has been superseded by base/policy/caller checks.

Unresolved premise or limitation: A supported missed replacement descendant and consequent damage remain unestablished. Independent drop check must agree with the whole-range attribution proposal while preserving this unresolved safety question.

Proposed basis: Outside the pinned range for this unchanged helper/policy/restoration route. Do not label the residual safe or specification-accepted.

Existing parent disposition (event 13): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: whole-range-attribution-unchanged-Warn-to-Ok-relaunch-unwind. Full065 base helper1177-1226, caller13755-13829 and sweep1539-1562 already carry this Warn-to-Ok definitive rollback;1ed helper733-782 and sweep1527-1543 preserve them, full-range policy diff and bounded history show no introduced change. Attribution only, not safety or acceptance. Concrete missed replacement descendant and consequent harm remain missing. Existing restart-unwind queue covers actual Err/ambiguous/no-terminal path and is not an alias. Do not invent a new Podman exposure without a named new producer or changed contract.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g099 record](records/review-through-00de0b6c/groups/g099.json). Record ID: `l1`.

