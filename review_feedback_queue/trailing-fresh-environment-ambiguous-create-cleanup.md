# Ambiguous environment launches can duplicate resources or discard a started agent (specification acceptance proposed)

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: An uncertain initial environment launch has two different consequences: after loss of the
client/helm response, explicitly launching again may create another environment; after loss of the supervisor's
session-create reply, the helm may destroy the new environment even if its agent already started. Original
duplicate-resource/session and extra Tensorlake billing concern retained. The latter can end that agent and discard
files it created before the lost acknowledgement, so it is retained as a potentially-highest finding for independent
specification-acceptance review, not silently dropped as a candidate. Source checks: create_form.rs:6072-6105 mints a
fresh key per submission; api/sandbox_sessions.rs:55-128 sends one 30-minute request through body decoding, maps
transport/header/decoding ambiguity to Unknown, and :9-12 directs ‘Check Hosts for the environment before launching
again.’ Helm sessions/sandbox.rs:61-106 owns the setup independently of the client response. Client response loss alone
does not initiate cleanup: a successful internal create can still finish normally. The destructive route is the internal
create error: provisioning/tensorlake_add.rs:991-1017 awaits create_in_session_sandbox then :1028-1038 passes every
error into forget_host_locked with the fresh host and existing provisioning guard. sessions.rs:1653-1691 routes that
initial create through create_session_request; :1940-1957 propagates the supervisor RPC error before acceptance, so lost
replies have no special success reconciliation here. hosts.rs:1470-1492 closes the environment connection and invoke

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **highest**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g086/l1`.

### g086/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 16 (finding, open):

Reviewed commit: 350a4b33b6d4476f4a78e43cea124bc2b828f462; relevant cleanup still present through
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR: An uncertain initial environment launch has two different consequences:
after loss of the client/helm response, explicitly launching again may create another environment; after loss of the
supervisor's session-create reply, the helm may destroy the new environment even if its agent already started. Original
duplicate-resource/session and extra Tensorlake billing concern retained. The latter can end that agent and discard
files it created before the lost acknowledgement, so it is retained as a potentially-highest finding for independent
specification-acceptance review, not silently dropped as a candidate. Source checks: create_form.rs:6072-6105 mints a
fresh key per submission; api/sandbox_sessions.rs:55-128 sends one 30-minute request through body decoding, maps
transport/header/decoding ambiguity to Unknown, and :9-12 directs ‘Check Hosts for the environment before launching
again.’ Helm sessions/sandbox.rs:61-106 owns the setup independently of the client response. Client response loss alone
does not initiate cleanup: a successful internal create can still finish normally. The destructive route is the internal
create error: provisioning/tensorlake_add.rs:991-1017 awaits create_in_session_sandbox then :1028-1038 passes every
error into forget_host_locked with the fresh host and existing provisioning guard. sessions.rs:1653-1691 routes that
initial create through create_session_request; :1940-1957 propagates the supervisor RPC error before acceptance, so lost
replies have no special success reconciliation here. hosts.rs:1470-1492 closes the environment connection and invokes
provider.destroy; only confirmed cleanup permits row removal. Thus this is confined to the resource created for this
initial launch, not a Replace source or arbitrary existing host. The exact accepted contract is explicit: SPEC.md
Sandbox sessions says ‘If an ordinary new launch fails before the helm has confirmed session creation, including a lost
create reply, it attempts to delete that newly created sandbox and snapshots’ and ‘This may end an agent that started
before its reply was lost.’ It also says ‘Transparent recovery of an ambiguous launch is not promised.’ SPEC_impl.md
Tensorlake sandbox sessions says ‘Lost create replies need no special reconciliation: the newly created resource may be
destroyed’ and ‘There is no new launch journal, retained request fingerprint, exactly-once launch guarantee or startup
continuation worker.’ SPEC.md Container sessions says ‘An ordinary failed initial launch may discard its newly created
container, including after a lost session-create reply.’ Failed cleanup retains its resource/reason for Hosts recovery;
see provisioning/tensorlake_add.rs:1039-1054. Suggested bucket: highest for the source-established
destruction-of-started-process/new-files consequence if not accepted; other for duplicate independent environments
alone. Confidence: confirmed conditional source route and exact specification acceptance, no execution or reproduction.
proposed_drop: explicit environment-specific specification acceptance in the exact quoted text, pending
independent/caller judgment; no parent drop accepted. possible_cover: none checked for this accepted concern.
Could-be-highest: yes, explicitly, for ending the started agent or discarding its newly created files; rarity does not
lower that impact. Do not conflate outer client response loss with internal supervisor reply loss or claim an observed
incident. Source premise is settled; no unresolved lead remains. Proposed action is independent acceptance review, not
an implementation fix contrary to the current contract.

Returned proposal (claims-056, event 16): **covered**, suggested bucket **highest**.

Both conditional consequences are source-supported and explicitly accepted: another explicit launch after outer response
loss can allocate another environment; an internal supervisor-create error can destroy this launch's fresh environment
even if its agent started. This is proposed specification coverage, not an authorized drop.

Original stronger consequence: Preserve both duplicate environments/sessions and extra Tensorlake billing after another
explicit launch, and ending an agent or discarding files it created after an internally lost session-create
acknowledgement. Cleanup is confined to this fresh resource, not an existing Replace source. Outer client response loss
alone does not initiate cleanup. Failed cleanup remains recoverable; no runtime incident was demonstrated.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Exact environment-specific specification acceptance, subject to the independent highest-priority drop
consultation.

Existing parent disposition (event 16): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: exact-initial-environment-ambiguity-contract-cover-two-transport-boundaries. Parent read
full056. Another explicit launch after outer client-response loss may allocate duplicate environment/billing; no
automatic retry. Separately internal supervisor-create error can trigger cleanup of this fresh resource after agent
started; outer response loss alone does not trigger that cleanup. Product/impl expressly permit preconfirmed initial
cleanup/lost create reply, no exactly-once or transparent continuation, including containers, and failed cleanup retains
remedy. Highest started-agent/file-loss consequence kept, not lowered; narrow new-resource scope never existing Replace
source. Fresh gate must judge both exact ambiguity clauses, not conflate boundaries or use no-recovery wording as
blanket loss permission.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g086 record](records/review-through-00de0b6c/groups/g086.json). Record ID: `l1`.
