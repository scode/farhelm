# Scoped-tab Delete policy gap predates the whole review range

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Scoped-tab Delete policy gap predates the whole review range

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g016/l4`.

### g016/l4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (candidate, open):

Reviewed commit: f96dd4363d97dc7e07dea5b8947c96be6bf912e8; full-range attribution checked at exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a and inclusive tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db.

## TLDR
Considered whether Delete could report cleanup finished without checking a tab’s recorded scope when the agent itself was unscoped and the supervisor’s manager probe is negative or still in its timeout cooldown. Source confirms the differing Delete policy, but both that implementation and the general fail-closed scope promise already exist at the exclusive range base. Do not report it as a new defect of this assigned terminology rewrite. This is an attribution-only proposed exclusion, not a claim that the existing behavior satisfies the contract.

## Details
At reviewed revision, SPEC_impl.md:4848–4855 says Stop/Restart/Delete/tab-close with recorded scope refuse during the one-minute probe cooldown and earn a re-probe after a definite negative. service/teardown.rs:402–450 instead collects tab units through ScopeUnits::extend_tab and re-probes directly only if entry.scope is present. service/session_hosts.rs:554–576 adds known scoped tab tags through the same extend_tab method. sweep.rs:1312–1315 keeps these names derived regardless of scoped=true, which affects only portable-sweep selection. :1465–1503 earns re-probe only from recorded/possible sets and skips derived names with no scope error when the manager is unusable. scope.rs:744–827 makes a timeout return unavailable for the cooldown and a definite negative stay unavailable without allow_reprobe; :1036–1055 makes units_matching fail when unavailable, which teardown explicitly ignores in that case. Thus an unscoped agent plus a scoped tab is not protected by the same manager-failure policy as a directly closed scoped tab. Source supports the narrow policy gap; an actual surviving scrubbed process was not executed or established as a concrete occurrence. Original suspected consequence remains deletion with an unchecked tab scope and possibly surviving processes, not proven work loss or security exposure.

Whole-range check: base SPEC_impl.md:5334–5343 already includes Delete in the same recorded-scope refusal/re-probe promises. Base teardown.rs:498–510 already routes host tags through add_tab_scopes and re-probes only for entry.scope. Base session_hosts.rs:553–585 already converts a true host tag into TabScope::Scoped then calls extend_tab; base sweep.rs:1316–1327 already stores that selection as derived, and :1477–1510 already skips derived names without scope errors for an unusable manager. The in-range 1ed482d change simplifies the selection enum to a bool while retaining this same policy; f96dd436 changes tmux-era examples to host tags without changing the general fail-closed claim. This is unlike a newly added false guarantee: the same general rule and its counterexample were both present before the entire reviewed range.

Revision closes l4 after checking caller preflights, manager caching/cooldown, tag-to-scope conversion and base-to-tip history. Suggested bucket: other for established cleanup/contract policy difference; no current security or work-loss route established. Confidence: confirmed source and attribution; no execution. proposed_drop: out-of-range attribution only, with exact base references above; no acceptance of the policy mismatch or parent-accepted drop is implied. possible_cover: none among supplied interface findings. Could-be-highest: no established highest route; surviving-process consequences would need a concrete supported producer and additional established harmful effect. No runtime tests, reproductions, live-install actions or coverage-source searches.

Returned proposal (claims-007, event 10): **out-of-range**, suggested bucket **other**.

Source supports the narrow manager-policy discrepancy, but the same host-tag-to-derived-name path and general fail-closed promise already exist at the exclusive range base. The proposed exclusion is attribution only; it does not accept the cleanup policy or erase its conditional consequences.

Original stronger consequence: Original lead suspected falsely confirmed Delete cleanup with an unchecked recorded tab scope and possibly surviving processes. The final candidate preserves that narrow policy gap, with no demonstrated work loss/security exposure, and proposes exclusion only because the implementation and governing promise predate the entire range.

Unresolved premise or limitation: Concrete surviving scrubbed process and an established harmful downstream effect remain missing for a stronger user-work/security consequence. A supported runtime occurrence was not reproduced. The source-level policy mismatch itself is established and pre-range.

Proposed basis: Out-of-range attribution supported by explicit exclusive-base implementation and product-contract references. Independent drop check must judge that narrow exclusion and preserve the surviving-process uncertainty; it must not treat this as source refutation, filter coverage, or acceptance.

Existing parent disposition (event 10): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: out-of-range. Full007sourceconfirmsrecordedtabhosttagsconvertedbyextend_tabtoderivednames, unavailablemanageromitsderivedreprobe/errorswhileonlyentry.scopegetsforcecheck. Narrowfailclosedpolicydiscrepancyreal; exactsamefullpathandproductscopecheckpromisealreadyexclusivebase, not merelyassignedparent. f96wordingdoesnotnewlyapplytohosttabsthatwerealreadygovernedbybasegeneralSPEC;1edenum→boolretainssemantics. Proposedattributionexclusiononly, preservesactualuncheckedscopes/possibleprocesssurvival andmissingactualscrubbedharmroute. No hungmanagerfilterbecausefalsecleanupsuccessoutsidethatfilter; freshcheckprebasecontractandcode, notabsenceprooforacceptance.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g016 record](records/review-through-00de0b6c/groups/g016.json). Record ID: `l4`.

