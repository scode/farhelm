# Proposed drop retained: cleanup may destroy only the initial launch's new resource

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: An agent whose first creation reply to the helm is lost can already have written work when failed-initial-launch cleanup destroys its own newly allocated environment. Preserve this potentially highest loss-of-work consequence and proposed drop for independent checking; no parent drop accepted. Exact original acceptance: SPEC.md 4764–4766 says, "Failed or ambiguous Replace preserves the container and files" and "An ordinary failed initial launch may discard its newly created container, including after a lost session-create reply." SPEC_impl.md 6576–6578 says Replace preserves existing files and "A failed normal initial launch may destroy only its new environment." SPEC.md 1465–1470 explicitly says failed ordinary new launch, including lost create reply, attempts deletion and "This may end an agent that started before its reply was lost." At pinned tip the same initial-launch/Replace distinction remains in SPEC.md 4831–4837. Source checks now establish the route rather than relying only on design: provisioning/tensorlake_add.rs 388–398 allocates a fresh reserved name and registers a new session-owned row, retains that returned host through environment_session_steps and finish_session_launch at 479–485, and passes that exact captured host to first-session creation at 1019–1025. sessions.rs 1761–1803 uses that captured session-owned host; its ordinary create path propagates a client create error at 2077–2094. finish_session_launch 1040–1050 invokes whole-resource cleanup on that same host for an error, expressly including a lost create reply; successful creation at 1030–1038 ta

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g003/f3`.

### g003/f3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 7 (finding, open):

Reviewed commit: c8bc155cc36723c8c53870944d07ce8ec61ef9ed; synthesis implementation revision: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: An agent whose first creation reply to the helm is lost can already have written work when failed-initial-launch cleanup destroys its own newly allocated environment. Preserve this potentially highest loss-of-work consequence and proposed drop for independent checking; no parent drop accepted. Exact original acceptance: SPEC.md 4764–4766 says, "Failed or ambiguous Replace preserves the container and files" and "An ordinary failed initial launch may discard its newly created container, including after a lost session-create reply." SPEC_impl.md 6576–6578 says Replace preserves existing files and "A failed normal initial launch may destroy only its new environment." SPEC.md 1465–1470 explicitly says failed ordinary new launch, including lost create reply, attempts deletion and "This may end an agent that started before its reply was lost." At pinned tip the same initial-launch/Replace distinction remains in SPEC.md 4831–4837. Source checks now establish the route rather than relying only on design: provisioning/tensorlake_add.rs 388–398 allocates a fresh reserved name and registers a new session-owned row, retains that returned host through environment_session_steps and finish_session_launch at 479–485, and passes that exact captured host to first-session creation at 1019–1025. sessions.rs 1761–1803 uses that captured session-owned host; its ordinary create path propagates a client create error at 2077–2094. finish_session_launch 1040–1050 invokes whole-resource cleanup on that same host for an error, expressly including a lost create reply; successful creation at 1030–1038 takes no such cleanup. Browser/result-receiver loss is different: the owned task completes, and its final result.send at 1069 is ignored if the receiver is gone; losing the outer browser reply alone does not destroy a successfully confirmed session. hosts.rs 1506–1553 resolves the captured row's binding, invokes that provider's destroy, and requires returned proof before forgetting it. Podman destroy 1292–1333 resolves only its recorded canonical ID or pending-create reserved name, checks ownership for recovered IDs, and retains unsettled absence. remove_container at 400–414 rechecks this helm/session labels, removes the inspected canonical ID, and confirms absence. No source route to unrelated resources was established by these checks. Replace has a separate path: sessions.rs finish_replacement 4547–4647 sends client.delete_session_with for only the accepted replacement's source at 4575; failures report definite/ambiguous source outcomes rather than invoking whole-resource cleanup. Suggested bucket: highest for the preserved agent-work loss consequence if unaccepted; could-be-highest: true. Confidence: source-confirmed implementation and explicit specification acceptance, not execution. proposed_drop: exact initial-new-resource acceptance quoted above, pending independent caller decision. possible_cover: none. The acceptance does not cover Replace's existing resource, any unrelated environment, or normal successful create followed only by browser disconnection. It does not authorize cleanup after confirmed creation. No runtime tests or reproductions run.

Returned proposal (claims-002, event 7): **covered**, suggested bucket **highest**.

The source permits destruction of the fresh launch's own resource after an ambiguous first create. Both specifications explicitly accept that exact potentially destructive consequence. This is only a proposed exclusion; it requires the independent drop check.

Original stronger consequence: An agent can already have written user work when the lost first-create reply causes cleanup to destroy its new environment. Preserve highest pending independent adjudication; Replace, unrelated resources and browser-only reply loss are excluded.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact product-spec acceptance, confined to ordinary failed initial launch and its own newly created resource.

Existing parent disposition (event 7): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: covered. Parentreadfull002. ExactSPECinitialordinarylaunchacceptance explicitlyincludeslostfirstsession-createreply/alreadystartedagent anddestructiononlyitsnewresource. SourceErr→destroycapturedhostmatches, outerreceiverlossaloneisnottrigger. Highestactualpossiblewrittenworkretained untilfreshAGREE onexactacceptance; Replace preservation/unrelatedresources/confirmedcreate excluded. No rarityfilter or runtimeobservation.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g003 record](records/review-through-00de0b6c/groups/g003.json). Record ID: `f3`.

