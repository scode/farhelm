# Legacy tmux server may survive uninstall; specification explicitly accepts it

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Legacy tmux server may survive uninstall; specification explicitly accepts it

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g057/l1`.

### g057/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 10 (finding, open):

# Uninstall can leave a previously orphaned private tmux server

Reviewed commit: 500548b9253b07a26fd03119679184b69544e31d

## TLDR

A legacy private tmux server that already outlived a stopped supervisor can keep running after remote uninstall,
including after removal of the host entry and its installed binary.

## Details

backend.rs:3303-3345 stops a service unless inactive/failed, then confirms and cleans terminal hosts, without the former
private-socket lookup or tmux kill-server. SPEC.md:214-217 says exactly: 'Uninstall has no tmux step of its own:
stopping the supervisor's service ends whatever still runs under it, an earlier release's private tmux server included,
but one that outlived a supervisor stopped before uninstall (on a host never updated to a release that ends it at
startup, say) keeps running, which the maintainer accepted on 2026-11-11.' Only source line wrapping removed.
SPEC_impl.md:7853-7863 includes the same residual after failed startup ending followed by later supervisor stop.

Synthesis scope clarification: this proposed acceptance applies only to the already stopped/orphaned private-server
residual on uninstall. It says nothing about permission to mutate an answering old supervisor during Add. Direct source
checks for that separate premise are in c4: Add discovery and confirmation adopt answering supervisors, including
protocol-skewed peers, and finish_use_as_is skips planned steps without executing cleanup. The accepted residual also
does not excuse failed terminal-host termination; backend.rs:3309-3339 still fails those checks before later lib
removal. Caller-supplied g048/c1, g047/c1 and g046/f1 identify the same orphan-server boundary; g047/c3 concerns fixture
evidence and is not evidence that active-unit or terminal-host cleanup actually ran.

Suggested bucket: other for the established residual-process cleanup consequence, preserving original possible highest
for independent determination; no lowering or drop has been accepted by the parent. Could-be-highest: yes, original
concern involved user-owned processes left unmanaged; no separate source route to loss of work or security impact has
been established. Confidence: confirmed source/contract conclusion, no execution. proposed_drop: exact specification
acceptance above, proposed only for the already orphaned server residual. possible_cover: cited contracts and
caller-supplied related group records only; excluded queue coverage not read. Material caveats: active control-group
termination remains a specified implementation contract, not observed behavior; ordinary Add adoption, update
replacement and terminal-host cleanup retain distinct obligations. No missing premise remains for the stated residual
and its exact acceptance.

Returned proposal (claims-036, event 10): **covered**, suggested bucket **other**.

The exact already-stopped-supervisor residual is explicitly accepted by both specifications, including a prior
startup-shutdown failure. The proposal does not waive active-unit or terminal-host termination.

Original stronger consequence: Initial lead considered owned processes left unmanaged and possible highest if uninstall
destroyed work beyond confirmation. Later events establish only legacy-server survival and propose exact-spec coverage;
potential highest and all prior proposed drops remain for independent review.

Unresolved premise or limitation: No missing premise for the exact accepted survival case. A distinct
loss-of-work/security consequence, failure to stop an active unit, or failed terminal-host termination would require its
own established route and would fall outside this proposal.

Proposed basis: Exact specification acceptance, limited to a legacy private server already orphaned before uninstall. No
parent disposition inferred.

Existing parent disposition (event 10): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-spec-accepted-already-orphaned-legacy-server-uninstall-survival. Full036 and parent
actual uninstall/spec passages accept precisely a private server already surviving its previously stopped supervisor,
including failed startup ending, after host/binary removal. Active unit and terminal-host confirmation remain mandatory;
failure stops later removal. Same046f1/047c1/048c1 residual, retain each history. No runtime cgroup proof or general
older-release filter, and no permission to apply Add cleanup to an answering supervisor. Preserve original possible
work/security framing for independent scope judgment.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g057 record](records/review-through-00de0b6c/groups/g057.json). Record ID: `l1`.
