# Retired container sessions deliberately lose Stop and terminal access

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Existing experimental container sessions become unavailable for terminal interaction, Stop,
Restart and Replace; Delete remains their only supported action. This can strand an existing running session from the
product's ordinary controls, but the restriction is intentional. Source basis: assigned SPEC_impl.md patch removes the
supervisor-outside launch, account, hooks and terminal mechanisms and replaces them with a decode-and-delete
compatibility path. At the pinned revision, SPEC_impl.md:8646-8656 retains decoding, legacy removal, confirmed absence
and creation/deletion lock handling, and permits removal of confinement/attach helpers only 'once nothing can create or
open these sessions.' Exact product acceptance, SPEC.md, Retired containerized sessions: 'Existing sessions support
Delete only. Opening terminals or tabs, exec, Stop, Restart, Clone, Replace and Replace with are refused.' Exact cleanup
requirement: 'Delete retains the legacy container-removal path and confirms absence.' The replacement ordinary container
model is specified separately, with an in-container supervisor; it does not silently reinterpret the old configuration
as a new launch. Suggested bucket if unintended: high/material loss of access to running sessions. Could-be-highest: no
established unconfirmed destructive route; an explicit Delete is still warned and requires confirmed removal, and this
documentation-only patch does not itself stop a running process. Confidence: confirmed explicit compatibility
restriction in both specifications; implementation effects not executed. proposed_drop: the exact

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **high**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g004/f3`.

### g004/f3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 6 (candidate, open):

Reviewed commit: c8bc155cc36723c8c53870944d07ce8ec61ef9ed. TLDR: Existing experimental container sessions become
unavailable for terminal interaction, Stop, Restart and Replace; Delete remains their only supported action. This can
strand an existing running session from the product's ordinary controls, but the restriction is intentional. Source
basis: assigned SPEC_impl.md patch removes the supervisor-outside launch, account, hooks and terminal mechanisms and
replaces them with a decode-and-delete compatibility path. At the pinned revision, SPEC_impl.md:8646-8656 retains
decoding, legacy removal, confirmed absence and creation/deletion lock handling, and permits removal of
confinement/attach helpers only 'once nothing can create or open these sessions.' Exact product acceptance, SPEC.md,
Retired containerized sessions: 'Existing sessions support Delete only. Opening terminals or tabs, exec, Stop, Restart,
Clone, Replace and Replace with are refused.' Exact cleanup requirement: 'Delete retains the legacy container-removal
path and confirms absence.' The replacement ordinary container model is specified separately, with an in-container
supervisor; it does not silently reinterpret the old configuration as a new launch. Suggested bucket if unintended:
high/material loss of access to running sessions. Could-be-highest: no established unconfirmed destructive route; an
explicit Delete is still warned and requires confirmed removal, and this documentation-only patch does not itself stop a
running process. Confidence: confirmed explicit compatibility restriction in both specifications; implementation effects
not executed. proposed_drop: the exact quoted specification deliberately makes these old experimental sessions
delete-only. possible_cover: none; no coverage sources consulted. No runtime tests or reproductions run.

Returned proposal (claims-002, event 6): **covered**, suggested bucket **high**.

The exact loss of normal terminal/lifecycle access to old experimental container sessions is expressly required by both
specifications. It is not authority to destroy without the retained Delete confirmation or to reinterpret them as new
environments.

Original stronger consequence: An existing running experimental container session can be inaccessible through ordinary
terminal and Stop/Restart/Replace controls. Original high; no unauthorized destructive route established. Preserve the
running-work context for the independent check.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact intentional delete-only compatibility rule; its existing running-work context warrants
conservative independent adjudication.

Existing parent disposition (event 6): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: covered.
Full002exactretiredexperimentalcontainerDelete-onlycompatibilitypolicyexpresslyrefusesallterminal/exec/Stop/Restart/Clone/Replaceaccess,
includingexistingrunningrows. Intentionallossnormalaccesscovered;
noauthorizationforunconfirmedDelete,wrongresource,ornewenvironmentreinterpretation.
Preserveliveworkcontext/potentialhighestquestionforfreshconsult; no runtimeclaim or broadcompatibilityexception.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g004 record](records/review-through-00de0b6c/groups/g004.json). Record ID: `f3`.
