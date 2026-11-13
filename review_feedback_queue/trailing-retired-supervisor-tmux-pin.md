# Removing the unit's tmux pin matches the retired runtime contract

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Removing the unit's tmux pin matches the retired runtime contract

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g044/c3`.

### g044/c3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 9 (candidate, open):

Reviewed commit: 17a3fe858c5511a24b7403af15dd41c93bf69fd7. TLDR considered: removing the exact approved tmux executable from the supervisor unit could reintroduce the below-floor private-binary shadowing problem documented in the removed comment. The replacement contract removes that supervisor use: SPEC_impl.md, Terminal substrate: private tmux server, says exactly: The running supervisor runs no tmux at all, and its driver of the old terminals is gone; it also says the unit names no tmux and puts no tmux directory on PATH. Its retained startup compatibility step explicitly works whether or not any tmux program exists and uses a plain socket connect plus pinned process signals. The assigned plan.rs patch changes supervisor_unit to the shared farhelm/state-only renderer while preserving the separately specified Linux provisioning payload check. Synthesis revision: this concerns Farhelm's own runtime selection only. It makes no claim that an inherited lib/tmux cannot be selected by a user's command or startup script. New evidence for the separately owned g045/l2 at 500548b9253b07a26fd03119679184b69544e31d: snapshot_preparer.rs:48-55 passes only target, unit name and state directory to finish_snapshot_install before publishing facts at 56-60; tensorlake_backend.rs:207-300 builds its final guard solely from unit, state, cgroup root and machine-id. Its final absence predicates at 292 check data/supervisor.db and data, not the private lib directory. Thus the publication guard does not fill the inherited-binary absence premise missing from the skip at snapshot_preparer.rs:109-119. This is source corroboration for g045/l2, not a new or duplicate local finding. Original possible shadowing consequence is preserved but separated into supervisor selection (refuted here) and explicit user/agent command resolution (owned by g045/l2). Suggested bucket: none for removing the unused supervisor pin. Could-be-highest: no established loss-of-work or security route for this pin removal; do not infer safety of unrelated inherited binaries. Confidence: confirmed by source/contract; no execution. proposed_drop: exact specification acceptance of supervisor pin removal only; parent has not accepted the proposed drop. possible_cover: g045/l2, supplied by parent, addresses the separate preparation removal skip; its finding is not refuted here.

Returned proposal (claims-027, event 9): **covered**, suggested bucket **other**.

The supervisor no longer runs tmux, and its legacy-server shutdown uses socket/process operations without executing tmux. Removing its runtime pin follows the explicit replacement contract. This does not establish that an inherited private tmux cannot be selected by a user's or agent's command/startup script; that separate preparation-removal question remains g045/l2's scope.

Original stronger consequence: Removing an approved exact tmux pin might reintroduce selection of an unapproved or below-floor binary. Supervisor selection is refuted; explicit user/agent command shadowing is a separate preserved concern owned by g045/l2.

Unresolved premise or limitation: No runtime-selection premise remains. Inherited private-lib absence is unproven by the final publication guard and must not be inferred from this result.

Proposed basis: Exact replacement contract accepts removing an unused supervisor tmux pin. Conservatively request independent checking because the original broader shadowing consequence touches user commands and must remain separated.

Existing parent disposition (event 9): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-no-tmux-supervisor-runtime-pin-retirement-with-command-shadowing-residual-separated. Full027 supervisor/unitrunsnonames/PATHtmux, socket/processoldcleanupworksnoprogramsoexactunusedruntimepinremovalaccepted. DoesNOTestablishinheritedprivate-libabsence: finalcleanupunit/stateinputandstate/databasechecksno-libpath, later500preparerskip explicit. Separate045l2 user/agentcommandshadowingretainedandalreadyprimaryreconciled; nooldruntimepincoverofthatroot. Originalpotentialunapproved/belowfloorselectionbroadconsequencestaysfreshgate, runtimerouteonly.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g044 record](records/review-through-00de0b6c/groups/g044.json). Record ID: `c3`.

