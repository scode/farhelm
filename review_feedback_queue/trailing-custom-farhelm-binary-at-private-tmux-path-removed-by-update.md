# A custom Farhelm executable at the reserved tmux path collides with cleanup

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: A custom Farhelm executable at the reserved tmux path collides with cleanup

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **high**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g048/l2`.

### g048/l2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 11 (candidate, open):

# Cleanup collides with a nonstandard executable path

Reviewed commit: 500548b9253b07a26fd03119679184b69544e31d

## TLDR

A host manually arranged so that Farhelm itself is installed as ~/.local/lib/farhelm/tmux can have that executable removed by Update after it is installed and restarted. This requires a nonstandard, best-effort installation layout; ordinary setup always installs the executable as farhelm.

## Details

Original potential harm preserved: Update can remove its own executable and break later connections or restarts at that custom path. Source checks: plan.rs:737-740 defaults Farhelm to lib_dir/farhelm, while plan_for_row at :1152-1174 accepts an absolute registered path and preserves it. The new unconditional removal at :867-869 names lib_dir/tmux. Registration's remote_farhelm_is_usable/storable at store.rs:2277-2292 permits that absolute basename, and Update discovery at service.rs:1385-1429 and :1493-1498 carries the probed executable path into the plan. Thus the coordinate collision is source-real for a custom install; it is not refuted by correct ordinary setup. Full-range attribution was checked: at exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a plan.rs:821-855, tmux installation was conditional on needs_tmux; a host with a sufficient own tmux avoided that overwrite. The unconditional removal is introduced by the reviewed patch. No claim is made that the collision predates the review range.

Exact acceptance boundary: SPEC.md:5345-5354, Supported host setup, says: "Every other way a host can end up with a supervisor works on a best-effort basis, mainly for people working on Farhelm itself: a supervisor started by hand with `farhelm supervisor run`, one installed with `install.sh`, one run by a unit the user wrote or changed with drop-ins, one at paths other than the layout setup installs, or a host provisioning does not target. Do not spend significant complexity making setup or Update detect, adapt to, coexist with, or preserve such setups; refusing with a clear message is enough." The reviewed change also explicitly reserves and removes that private tmux path (SPEC.md:182-188). No ordinary setup or shipped control that places Farhelm at the reserved tmux coordinate has been found.

Suggested bucket of original consequence: high if reported, since the nonstandard host can lose its reachable installation. Could-be-highest: no established loss of user work or security route; the executable is replaceable and terminal-host survival is separate. Confidence: source-confirmed collision and nonstandard-layout premise, no execution. proposed_drop: the exact nonstandard-install preservation exclusion above; this is a proposal only and no parent drop/lowering is accepted. possible_cover: none; neighbour novelty input contains no matching cause, and no prohibited queue/coverage sources were read. Caveats: the contract does not promise preserving that layout; supported setup uses lib_dir/farhelm. This candidate does not excuse deletion of unrelated files outside Farhelm's private layout, or a collision reachable with the supported default layout. Lead l2 is resolved to this candidate, with original harm and provenance retained.

Discovery follow-through: the complete diff of plan.rs from the reviewed commit through inclusive endpoint 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db is empty, so this source collision remains at the endpoint. The conditional tmux install at the exclusive base establishes that the collision did not necessarily occur before this range on a host with adequate system tmux; the report therefore retains attribution to this new unconditional removal. The source-supported nonstandard-layout exclusion is the only proposed-drop basis; rarity and correct production defaults are not used as independent refutations. Required contract section SPEC.md:5343-5376 was also delivered through the evidence helper. No missing readily checkable premise remains for the stated custom-coordinate collision, and no runtime incidence is claimed.

Returned proposal (claims-030, event 11): **covered**, suggested bucket **high**.

The collision is real for the stated manually arranged path. The proposed omission rests solely on explicit nonstandard-layout preservation exclusion and ownership of the private tmux coordinate; ordinary default correctness and rarity do not refute it.

Original stronger consequence: Update can remove the executable it just installed, breaking later connections and restarts; the initial lead contemplated highest if a further user-work loss route existed. Later records preserve the source-real collision and propose only the nonstandard-layout exclusion.

Unresolved premise or limitation: No supported setup path placing Farhelm itself at private lib/tmux was established. Breaking later connections/restarts is supported for the custom layout; additional loss of sessions or other user work remains unestablished.

Proposed basis: Exact nonstandard-layout preservation exclusion plus explicit ownership/removal of private lib/tmux. Independent drop check must judge that narrow exclusion; source does not refute the collision and does not excuse deletion outside Farhelm's private layout.

Existing parent disposition (event 11): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: nonstandard-reserved-private-coordinate-coverage-pending. Parent read full030 plusactualSPEC5320-5345/178-220andimpl7990-8025. Realinrangeunconditionalremove lib/tmuxcanunlinkjustinstalledFarhelmmanuallyregisteredatthatsamereservedprivatepath; defaultcorrectnessnotrefutation. Proposedcoverrequirescombiningexplicitownedprivatepathremovalandbest-effortnonstandardsetup/significantcomplexityexclusion. 'refusing with a clear message is enough' doesnotitselfsayeverysilentbrokenresultaccepted; freshchecker mustjudgeexactcustomreservedpathscope, ambiguityretainsrecord. Noordinarysupportedproducerestablishedforthisplacement, laterconnection/restartfailurehighestablished, userworklossadditionalunestablished. Preserveinitialhighestpossibilitythroughgate, neitherraritynorordinaryuserbinarydeletionexemptionoutsideprivatepath.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g048 record](records/review-through-00de0b6c/groups/g048.json). Record ID: `l2`.

