# Fresh Claude trust lacks the documented birth-time check

Reviewed commit: ab96eaeab804318d1f380b159b49c170218b4250

## TLDR

Initial fresh-checkout creation can edit Claude trust without the no-birth-time check stated elsewhere. Fresh-creation
rules may intentionally be an exception; this is a source/specification conflict, not an established unauthorized
replacement-folder grant.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-supervisor/src/service/core.rs:10037`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Follow-up evidence for d1, reviewed ab96eaeab804318d1f380b159b49c170218b4250. The ordinary opt-in fresh-launch path is
source-established: allocation captures target_observed.birth_ns at working_copies.rs1414-1418 and stores it
at1464-1479; sessions.rs1169 forwards configured Claude trust; core.rs10037 passes it after allocation;
launch.rs1873-1889 resolves and edits the vendor trust record. agent_kind/claude/trust.rs81-104 canonicalizes the
project path without checking its birth evidence, and174-249 writes hasTrustDialogAccepted=true under the vendor lock
when the configuration exists and is valid. Config absence or edit failure prevents this observed consequence; include a
preexisting usable Claude config and successful edit in the conditional trigger. Pinned-tip snippets confirm persistence
at helm sessions.rs1190, supervisor core.rs10071 and launch.rs1873-1889. An overbroad intervening diff was clipped and
discarded; none of its missing content supports the conclusion. Confidence confirmed conditional behavior, not measured
runtime. Contract ambiguity remains: SPEC.md1128-1131/ledger18202-18212 say no automatic trust when no birth time, while
SPEC_impl.md2406-2408 treats fresh intent as ownership. Retain as other policy discrepancy with possible_cover=ambiguous
fresh-creation exception and proposed_drop=false. No unauthorized replacement/foreign-content route has been shown for
this initial launch, so a separate highest security consequence is unestablished, not merely unmeasured. No claim about
installed vendor behavior or a runtime reproduction. Original known findings remain unchanged.

Provenance: `a044/d1`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
