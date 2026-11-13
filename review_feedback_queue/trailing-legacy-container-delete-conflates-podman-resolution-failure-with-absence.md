# unverified after investigation: PATH lookup failure versus Podman absence

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: deleting a retired container session can skip container removal and forget its handle when the supervisor cannot find Podman although the engine may remain installed. Original possible consequence retained: orphaned container storage or user-owned processes/work. No process survival, credential exposure or extra loss of work beyond the requested Delete is established.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket does not approve any proposed lowering.

Contributing records: `g095/l1`.

### g095/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: 04d85f0c98a9b635709d00b056719a0baa7fd3fc.

TLDR: deleting a retired container session can skip container removal and forget its handle when the supervisor cannot find Podman although the engine may remain installed. Original possible consequence retained: orphaned container storage or user-owned processes/work. No process survival, credential exposure or extra loss of work beyond the requested Delete is established.

Details: container.rs:170-190 treats None from scope::resolve_program as successful cleanup. scope.rs:1233-1281 returns None for missing/unusable PATH entries and any metadata failure, not proof of installation-wide absence. teardown.rs:570-590 revokes creation under its permanent flock, then runs ordinary process cleanup; :999-1002 deletes the row if the other steps pass. sweep.rs:1410-1564 checks processes/scopes and does not remove or inspect Podman storage. Creation revocation prevents future ticket consumption but does not prove removal of existing state. Existing stopped-container storage is the narrow cleanup consequence; strongest process/work consequences remain conditional.

Discovery-turn premise resolution requested by parent: checked actual pre-retirement producer, not just CreationTicket's type. 04d85f0c^ service/core.rs:9553-9557 calls container::preflight in supervisor request validation. More decisively, :17271-17303 constructs ContainerRuntime and calls container::executable at :17276 inside the supervisor's launch construction, before serializing LaunchSpec or starting the shim. Parent container.rs:101-107 resolves through scope::resolve_program; publish at :500-509 stores that already-resolved runtime in CreationTicket. Therefore the absolute Podman path came from the old supervisor's own inherited PATH, not an agent login shell. The agent login environment cannot supply a hidden engine that the same supervisor could not initially resolve. This disproves that proposed ordinary same-startup shipped producer; it does not establish PATH stability across a later supervisor restart, or rule out a later metadata inspection error. No supported automatic transition changing that PATH has been demonstrated.

Current startup sources checked in the previous turn (main.rs:2172-2199; core.rs:6345-6383,6429-6520,18851-18900) neither read the old ticket's executable nor restore a previous Podman PATH. Complete supervisor/main set_var search found no process PATH replacement. A supported deployed environment's historical PATH transition is still not established from that fact alone. The old ticket's absolute path is discarded by revocation, but using it would be a possible implementation choice, not an established requirement.

Exact acceptance retained: SPEC.md:4820-4821 says 'Delete retains the legacy container-removal path and confirms absence. Missing Podman counts as nothing left to remove.' SPEC_impl.md:8652-8654 says 'Resolve Podman lazily. A host without Podman has nothing left to remove. Other unconfirmed cleanup keeps the session for retry.' This wording persists at the second assigned commit. scope.rs:1245-1257 explicitly treats unsupported non-UTF-8 tool locations as absent. No literal rule found equates every omitted PATH entry or metadata failure with a host without Podman. Hand-edited units and manual remote supervisor setups are best effort under SPEC.md:5251-5283, so such edits are not asserted to prove a guaranteed automatic-upgrade regression.

Suggested bucket: other for unconfirmed resource cleanup and record loss. Could-be-highest: original orphaned-process/work possibility retained for independent judgment, but not established; Delete already authorizes destruction of the container and its internal files, and later process cleanup may stop it. Confidence: confirmed source route and newly resolved producer premise; unverified after investigation as a reportable contract violation. Exact missing premises remain whether 'Missing Podman' intentionally includes every pre-spawn lookup failure, and a supported deployment retaining a legacy container while hitting an unaccepted failure. Source proves old supervisor-PATH origin, not historical deployment environments or the maintainer's unstated interpretation. Retained deployment PATH/container evidence or an explicit acceptance interpretation would settle those premises; no reproduction requested/performed.

proposed_drop: partial only for genuine engine absence and documented unsupported non-UTF-8 locations under the exact quoted exceptions; no blanket drop of PATH/metadata ambiguity accepted by parent. No lowering of established strongest impact. possible_cover: discovery-neighbours.json provides g001/f2 with the same lazy missing-engine cleanup/row-loss route, likely duplicate; preserve l1 for parent reconciliation. Adjacent old-terminal guarded-Delete findings are distinct, already recorded by their owners, and are not new discoveries here. All prior source references, consequences and caveats remain preserved in this event history. No new ID or newly verified bug in this premise check; source-only analysis, no execution.

Additional declared startup-PATH check this discovery turn: units.rs:57-60,94-106,132-160 explicitly composes the supported Linux supervisor service PATH from the directories holding Farhelm and the pinned tmux, followed by /usr/local/bin:/usr/bin:/bin. It does not inherit arbitrary agent login-shell PATH. Ordinary creation already requires Podman to resolve in that old supervisor PATH (parent core.rs:17274-17276), so an engine available only in an agent login shell is not a shipped producer. No change of the Farhelm/tmux directories or engine location was established. This source narrows, rather than supplies, a supported trigger for l1. Range attribution check: git log of container.rs across the whole exclusive-base/inclusive-tip range shows only the retirement and later comment-only tmux-removal commits; the base-to-retirement-parent diff for this file is empty. No later in-range repair or additional assigned edit was omitted.

Returned proposal (claims-063, event 12): **retain-unverified**, suggested bucket **other**.

The cleanup branch is real, but the latest record correctly leaves its supported trigger and contract interpretation unresolved. Genuine missing Podman is explicitly accepted; failure to resolve an installed engine is not expressly distinguished in the specifications. Retain that uncertainty rather than turning either interpretation into a fact. The reported g001/f2 overlap is outside this supplied batch and was not independently checked.

Original stronger consequence: The initial suspicion was that forgetting a retired session could orphan its container and user-owned processes/work. The later records preserve that possibility but establish only conditional unconfirmed storage cleanup and row loss. Creation under an agent-only login PATH was source-refuted; subsequent supervisor PATH changes or metadata failures remain unobserved. No accepted blanket withdrawal exists.

Unresolved premise or limitation: A supported deployment retaining an old container while hitting a lookup failure outside the intended missing-Podman acceptance; whether that acceptance intentionally includes PATH omissions and inspection failures. A stronger bucket additionally needs an established loss route beyond the destruction Delete already authorizes; process survival, credential exposure and extra work loss were not established.

Proposed basis: Retain the unresolved remainder. Any dismissal of genuine absence or unsupported non-UTF-8 locations, application of the upgrade filter, or exclusion of the possible process/work consequence still goes to the parent's independent drop register. This consultation is not that approval.

Existing parent disposition (event 12): under-investigation. Record durably acknowledged; parent source/contract validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: retain-investigated-installed-Podman-resolution-versus-absence-ambiguity. Parent reread063 in complete095 and096 chunks after clipped combined output. ResolverNone can mean PATH/metadata/unusable/nonUTF8, skipping rm/exists and settling marker/row; genuine absent engine explicitly accepted, other unconfirmed cleanup should retain. Original creation resolved before agent login shell, so login-onlyPATH producer refuted; supported unitPATH narrows but does not establish future stability or actual lookup-failure deployment. Retain investigated contract/trigger uncertainty, proposed other conditional unconfirmed storage cleanup, not confirmed orphan/security/workloss. Existing001f2 likely same root must reconcile actual history. Nonstandard hand-edits/nonUTF8 explicit limits and upgrade-plus-rare filter cannot be blanket cover without a supported transition. Original possible orphaned processes/work stays for fresh gate.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations: [g095 record](records/review-through-00de0b6c/groups/g095.json). Record ID: `l1`.

