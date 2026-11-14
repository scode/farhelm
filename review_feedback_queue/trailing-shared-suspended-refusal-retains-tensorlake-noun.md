# Paused-container refusals incorrectly identify a Tensorlake sandbox

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: A refused action on a paused Podman container says its host is a Tensorlake sandbox, misleading
the user about the resource that needs resuming.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g024/c4`.

### g024/c4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: 56659839785691f76da3af5d8f51b511cff4ba8b. TLDR: A refused action on a paused Podman container says its
host is a Tensorlake sandbox, misleading the user about the resource that needs resuming. Details: sessions.rs:1005-1011
hardcodes 'the host's Tensorlake sandbox is suspended' for the shared Suspended state. In this commit's new Podman
provider, environment/podman.rs:1258-1260 maps Podman's ordinary paused status to EnvironmentStatus::Suspended;
manager.rs:3819-3840 publishes the same HostState::Suspended for either provider. route_session at sessions.rs:904-925
returns refusal_text whenever this cached session's host has no connected client. An ordinary concrete route is POST
/api/sessions/{id}/tabs: open_tab at 4417-4427 routes through that refusal. A user opening a tab as the container
becomes paused, before the page receives the new host state, gets this incorrect provider noun; an authenticated API
request against a paused container does too. Rename and Restart likewise use route_session (3310 and 3159).
Changed-source attribution: the literal existed at parent sessions.rs:1003-1007 when it described Tensorlake hosts;
56659839785691f76da3af5d8f51b511cff4ba8b introduces paused Podman environments into this same status/refusal path, and
edits the neighboring lifecycle-changing sentence to 'environment' but leaves Suspended Tensorlake-specific. Thus the
regression is the newly shared call path rather than a newly introduced literal. Contract: SPEC.md Container sessions
(4737 onward) specifies distinct containers using Podman pause and provider-appropriate wording; SPEC_impl.md Disposable
environments says public wording is provider-owned and state is shared. A provider-neutral noun here, or dispatch
through the exhaustive wording boundary, would describe both accurately. Suggested bucket: other (diagnostic
correctness). Confidence: confirmed from source; no execution/reproduction. proposed_drop: false. possible_cover: none.
Could-be-highest: false; no wrong-provider mutation, security consequence or loss of work is claimed. Revision: replaces
the prior proposed drop. The prior record's original consequence (incorrect provider naming) remains; correct runtime
dispatch and small impact do not refute that wording defect. The original concern about actually selecting the wrong
lifecycle provider remains refuted by the stored-binding dispatch; that does not justify dropping this narrower
diagnosis. Attribution follow-up: git diff --name-status confirms environment/podman.rs is added by this exact commit;
lib.rs at 2459-2493 installs its new Podman service in the production Providers registry, replacing Tensorlake-only
assembly. This establishes the new shipped route that makes the inherited noun incorrect; it is not merely hypothetical
use of an old generic state.

Returned proposal (claims-013, event 12): **retain-verified**, suggested bucket **other**.

The refusal really names Tensorlake for a paused Podman container. Correct provider dispatch does not repair that
diagnostic. Neither a rare timing window nor unsupported client input is necessary for the authenticated API route, so
the rare-diagnostic filter does not fully match.

Original stronger consequence: An early investigation considered wrong-provider lifecycle dispatch. Stored-binding
dispatch refutes that route. The first proposed low-impact drop was expressly replaced by retention of the real naming
error.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain the diagnostic. Include the earlier wrong-resource-operation refutation in the parent's drop
register if it is treated as a possible user-work consequence; this consultation does not authorize that drop. g024/c5
supplies related enum documentation, not exact coverage of this user-facing refusal.

Existing parent disposition (event 12): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: retain-verified-other-noun-with-original-dispatch-refutation. Parentreadfull013.
NewlyreachablepausedPodman→SuspendedsharedAPIrefusaldeterministicallynamesTensorlake;
correctstoredbindingproviderdispatchdoesnotrepairdiagnostic. OrdinarysupportedAPIinputnotrare timing/unsupportedtrigger,
so rare-diagnosticfilterdoesnotfullymatch. Retainotherwordingdefectandtest/commentcontext;
separatelyoriginalwrongprovideroperationroutesource-refuted byexhaustivebinding/lifecyclePodmanresume,
preservepossibleuserprocessharmforfreshgate. Relatedc5enumdocsdifferenttextsurface, notfullcover.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g024 record](records/review-through-00de0b6c/groups/g024.json). Record ID: `c4`.
