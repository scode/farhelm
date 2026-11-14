# Failed initial launch may destroy work started by a lost create reply (explicitly accepted)

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: if session creation runs remotely but its reply is lost, the fresh environment can be destroyed
with the session and its work. This consequence was already present in run_tensorlake_add and is retained in extracted
finish_session_launch, then shared with provider-based launches. The error arm calls hosts::forget_host_locked for this
launch's fresh host. Contract: SPEC_impl.md, Tensorlake sandbox sessions, lines 6982-6987 states: 'Before a confirmed
create reply, a failed normal launch attempts whole-sandbox cleanup through the existing bounded Tensorlake deletion
calls and resource proof. The row is retained if cleanup cannot complete. Lost create replies need no special
reconciliation: the newly created resource may be destroyed.' SPEC_impl.md, same section, lines 6997-7002 separately
requires Replace to stay in its existing resource. Suggested bucket: highest if treated as a defect, because an executed
agent and its work may be destroyed. Could-be-highest: yes, explicit source route to destruction after ambiguous session
create; no additional user interaction premise required. Confidence: confirmed source consequence, not execution.
proposed_drop: explicitly accepted by the exact specification text above; retained as a finding for caller's independent
drop check. possible_cover: SPEC_impl.md Tensorlake sandbox sessions. Caveats: this concerns only the new resource owned
by this launch, never a Replace source; no reproduction performed; the cleanup behavior itself predates this patch.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g045/f1`.

### g045/f1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 2 (finding, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b. TLDR: if session creation runs remotely but its reply is
lost, the fresh environment can be destroyed with the session and its work. This consequence was already present in
run_tensorlake_add and is retained in extracted finish_session_launch, then shared with provider-based launches. The
error arm calls hosts::forget_host_locked for this launch's fresh host. Contract: SPEC_impl.md, Tensorlake sandbox
sessions, lines 6982-6987 states: 'Before a confirmed create reply, a failed normal launch attempts whole-sandbox
cleanup through the existing bounded Tensorlake deletion calls and resource proof. The row is retained if cleanup cannot
complete. Lost create replies need no special reconciliation: the newly created resource may be destroyed.'
SPEC_impl.md, same section, lines 6997-7002 separately requires Replace to stay in its existing resource. Suggested
bucket: highest if treated as a defect, because an executed agent and its work may be destroyed. Could-be-highest: yes,
explicit source route to destruction after ambiguous session create; no additional user interaction premise required.
Confidence: confirmed source consequence, not execution. proposed_drop: explicitly accepted by the exact specification
text above; retained as a finding for caller's independent drop check. possible_cover: SPEC_impl.md Tensorlake sandbox
sessions. Caveats: this concerns only the new resource owned by this launch, never a Replace source; no reproduction
performed; the cleanup behavior itself predates this patch.

Returned proposal (claims-028, event 2): **covered**, suggested bucket **highest**.

The source can destroy the new environment after a session started but its create reply was lost. Both current product
and implementation specifications explicitly accept this exact initial-launch cleanup consequence, including containers.

Original stronger consequence: A fresh environment, a remotely started session and its work may be destroyed when
session creation succeeded but its reply was lost. This consequence remains established and is not severity-lowered.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact explicit specification acceptance, limited to this launch's new resource before confirmed
creation. This is a proposed covered disposition only; an independent fresh-context drop check must agree before
omission.

Existing parent disposition (event 2): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-spec-accepted-initial-environment-lost-create-reply-cleanup. Full028
sourceconfirmsnewresourcecanbedestroyedafteragentstartsbutcreate-replylost; sharedinitialfinishonlynewresource,
neverReplacesource.
Exactproductandimplacceptpreconfirmednormalnewlaunchcleanupincludinglostreplyandcontainerinheritance,
retainrowifcleanupfails. Originalworklosskept highest, notrefuted/lowered.
Freshgatepreciseinitial-resource/before-confirmationboundary, noacceptanceolderexisting/Replaceworkloss.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g045 record](records/review-through-00de0b6c/groups/g045.json). Record ID: `f1`.
