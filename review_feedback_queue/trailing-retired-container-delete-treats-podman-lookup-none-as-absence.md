# Unverified after investigation: lookup failure exceeds the literal missing-Podman exception

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: deleting a retired container session when the supervisor cannot find Podman can report success
and remove the session record while the named container and its stored filesystem have not been removed or even checked.
Reinstalling or restoring Podman may expose that leftover, now without its Farhelm row. Suggested bucket: other for
established unconfirmed resource cleanup; confidence confirmed source route, no execution. Could-be-highest: original
possible loss of access to work or lingering credential-bearing processes remains explicitly retained for independent
judgment, but no security or work-loss route has been established. The users Delete already authorizes destruction of
that container; failure to destroy its internal files alone does not demonstrate extra loss of work. The managed
checkout remains on the host and takes ordinary archival/release handling, not the new environment Abandon disposition.
Source: pinned-tip container.rs:170-190 resolves Podman and skips rm plus container-exists verification on None, then
clears the initialization marker; scope.rs:1234-1284 shows None means no usable executable found in the supervisors
PATH, not proof of an empty Podman storage. Teardown.rs:451-467 derives the legacy helper scope, :523-540 revokes
creation, calls that removal, and still performs independent fail-closed process cleanup; :676-806 archives or releases
managed checkout membership and :814-817 deletes the session row. Original creator at policy revision
container.rs:327-377 creates a named detached container without --rm, with a host checkout bind. Consequently r

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g001/f2`.

### g001/f2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 13 (finding, open):

Reviewed policy commit c8bc155cc36723c8c53870944d07ce8ec61ef9ed. Implemented in
04d85f0c98a9b635709d00b056719a0baa7fd3fc; checked through pinned tip 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db. TLDR:
deleting a retired container session when the supervisor cannot find Podman can report success and remove the session
record while the named container and its stored filesystem have not been removed or even checked. Reinstalling or
restoring Podman may expose that leftover, now without its Farhelm row. Suggested bucket: other for established
unconfirmed resource cleanup; confidence confirmed source route, no execution. Could-be-highest: original possible loss
of access to work or lingering credential-bearing processes remains explicitly retained for independent judgment, but no
security or work-loss route has been established. The users Delete already authorizes destruction of that container;
failure to destroy its internal files alone does not demonstrate extra loss of work. The managed checkout remains on the
host and takes ordinary archival/release handling, not the new environment Abandon disposition.

Source: pinned-tip container.rs:170-190 resolves Podman and skips rm plus container-exists verification on None, then
clears the initialization marker; scope.rs:1234-1284 shows None means no usable executable found in the supervisors
PATH, not proof of an empty Podman storage. Teardown.rs:451-467 derives the legacy helper scope, :523-540 revokes
creation, calls that removal, and still performs independent fail-closed process cleanup; :676-806 archives or releases
managed checkout membership and :814-817 deletes the session row. Original creator at policy revision
container.rs:327-377 creates a named detached container without --rm, with a host checkout bind. Consequently removing
the Farhelm row does not itself remove container storage. No in-range repair: container.rs history shows the retirement
implementation and later tmux cleanup, and the skip remains at tip.

Highest-premise checks: sweep.rs:1448-1550 kills available named scopes and refuses recorded-scope or sweep failures;
container deletion is not the only process cleanup. The retired helper scope is included, and its deletion may stop
surviving container work. Thus a missing executable alone does not establish a running orphan or credential exposure. A
route to highest would require evidence of a legacy container process surviving successful ordinary scope/process
cleanup, or user work becoming inaccessible beyond the deletion the user authorized. Source inspected does not establish
either; no reproduction was requested or performed. This record reports the confirmed cleanup omission, retaining those
original possible consequences instead of calling them refuted.

Compatibility: pinned-tip SPEC.md:5373-5378 says compatibility is decided per feature and is not blanket permission for
destructive migrations or ordinary runtime data loss; :5434-5436 distinguishes upgrade mechanics from stored-data
survival. SPEC_impl.md:8328 onward binds values handed to running sessions across updates, while stored state follows
its own migration/adoption rules. These general contracts do not erase the feature-specific retirement policy, but do
not independently prove a highest consequence either.

proposed_drop: explicit narrow specification acceptance ONLY, retained as a finding for independent checking, not
dropped. Exact original policy text SPEC.md:4820-4821: Delete retains the legacy container-removal path and confirms
absence. Missing Podman counts as nothing left to remove. Exact current text at pinned tip SPEC.md:4895-4897: Existing
sessions support Delete only. Opening terminals or tabs, exec, Stop, Restart, Clone, Replace and Replace with are
refused. Delete retains the legacy container-removal path and confirms absence. Missing Podman counts as nothing left to
remove. SPEC_impl.md:8322-8324: Resolve Podman lazily. A host without Podman has nothing left to remove. Other
unconfirmed cleanup keeps the session for retry. possible_cover: those exact explicit exceptions only; no forbidden
coverage sources searched.

Original wording: Check retired-container Deletes missing-Podman exception; concern: loss of the row without removal,
possibly losing access to work or leaving credential-bearing processes running. Prior latest wording Missing-Podman
retirement policy has no shipped unsafe-deletion route here and its source-refuted proposed drop were invalid as a
complete judgment of this newly specified policy. Latest wording above preserves the original consequence and replaces
that reason with the actual later source route and exact specification exception. No established highest consequence was
lowered; independent check should judge the explicit exception and any potential-highest disposition. f2 lead is closed
as this retained finding; no unresolved readily checkable premise remains for its confirmed cleanup route. Citation
correction after checking numbered context: SPEC.md:5434-5436 is the exact floor-versus-data passage. Additional
compatibility premise: current core.rs:3479-3488 refuses retired launches through require_active_launch while retaining
Delete, and :9558-9567 preserves a retired row on interrupted-create retry. Thus neither later retry nor ordinary
opening constitutes resource rediscovery after successful Delete; the narrowly specified no-Podman branch remains the
relevant record-loss route. No new claim of work loss or process survival is made. Turn-4 named g095/l1 synthesis:
discovery-neighbours exact supplied record is the same lazy-cleanup/row-loss route; do not count as a new finding. The
literal acceptance is a host without Podman, not every reason resolve_program returns None. Exact inspected source at
tip scope.rs:1264-1284 skips any metadata error, omitted PATH entry, relative entry, nonregular/nonexecutable candidate
and unsupported non-UTF8 path. Therefore proposed_drop is now PARTIAL: genuine engine absence and explicitly unsupported
tool locations under cited policy, not automatic acceptance of all installed-engine lookup failures. The previous
blanket specification-acceptance proposal is narrowed, with its original wording preserved above. No parent
drop/lowering accepted. Source checks of actual earlier producer at 04d85f0c^ core.rs:17271-17303 establish
container::executable runs in the supervisor before publishing; container.rs:104-107 resolves once before shell startup
and :500-509 saves that runtime absolute path into the old ticket. Thus agent login-shell PATH does not supply an
ordinary hidden-engine producer. units.rs:57-60,94-106,132-160 builds supported Linux supervisor PATH from executable
directories and /usr/local/bin:/usr/bin:/bin. Base-to-retirement-parent container.rs diff is empty. Those checks narrow
the trigger; no repository path was shown to make a formerly resolvable supported Podman disappear from the restarted
supervisor PATH while leaving storage. Revoking the old ticket removes its saved absolute path but proves only
future-creation exclusion, not existing storage removal. Confidence: confirmed lookup/removal source behavior;
unverified after investigation as a REPORTABLE contract violation beyond the exact accepted absence case. Original
row-loss, orphaned storage and possible highest process/work consequences are preserved. Exact missing premises: a
supported deployment or non-deliberate metadata failure retaining legacy storage while hitting a lookup-None not
intended by Missing Podman, and the authoritative meaning of that phrase for those failures. Source establishes no
ordinary producer of the deployment transition and cannot settle unstated policy interpretation. Existing
deployment/startup PATH and lookup-error evidence, or explicit clarification that all lookup failures are intended as
engine absence, would settle it. No reproduction, local environment inspection or forbidden coverage search performed.
Suggested bucket remains other for confirmed cleanup omission; potential highest impact is neither established nor
discarded. No open uninvestigated lead remains; these are retained investigated caveats for parent judgment.

Returned proposal (claims-001, event 13): **retain-unverified**, suggested bucket **other**.

The lookup-None removal bypass and eventual row deletion are established. Genuine missing-Podman cleanup is expressly
accepted. The remaining reportable scope is an investigated uncertainty: whether supported installed-engine lookup
failures also reach None while retaining storage, and whether the narrow acceptance intends to include them. Do not
silently drop that remainder.

Original stronger consequence: Losing the Farhelm row while the named container survives could lose access to user work
or leave credential-bearing processes. The early source-refuted withdrawal was superseded by the later in-range
implementation; the later blanket acceptance proposal was narrowed to genuine absence and explicitly unsupported
locations. Preserve both changes and the unresolved stronger consequences.

Unresolved premise or limitation: A supported deployment transition or non-deliberate metadata failure that leaves
legacy storage while producing lookup-None outside the intended missing-Podman case; and the authoritative meaning of
missing Podman for such failures. Highest additionally requires surviving credential-bearing processes despite
successful ordinary process cleanup, or lost access to work beyond the destruction Delete already authorized. Earlier
reviewer checks of the old creator and installed supervisor PATH found no ordinary hidden-engine producer; those limits
remain.

Proposed basis: Retain the investigated installed-engine/lookup ambiguity. The explicit exception is a proposed partial
cover only for genuine absence and unsupported tool locations. Any acceptance-based drop or lowering of the original
possible highest consequences needs the independent drop check.

Existing parent disposition (event 13): under-investigation. Record durably acknowledged; parent source/contract
validation, cross-group synthesis, coverage/filter matching and any independent drop check remain pending. This
preserves the claim and uncertainty without accepting, dropping or lowering it.

Unapplied parent proposal: retain-unverified-other-with-partial-cover. Parent read full flagged001 and complete
actualownerf2. New04d lookupNone skipsrm/absencecheck anddeletesrow;
actualcontractacceptsgenuinemissingPodman/unsupportedlocations.
Broaderinstalled-but-unresolvableprogram/metadatafailureoutsideintendedexceptionremainUAI
afteractualcreator-ticket/supervisorPATH/processsweepchecks. Noordinaryhiddenengineproducerfound; preservethislimit and
noinventedstrongerrunningorphan because failclosedprocesscleanupindependent. Proposedpartialcovergenuineabsence only;
retaininvestigatedscopeambiguity/conditionalstorageleftover with allmissingpremises. Originalsecurity/worklossroute
needsprocesssurvivesuccessfulsweep or extraaccessloss beyondauthorizedDelete, notmerelyundeletedfiles.
Finalfreshcheckjudgepartialacceptance andoutsidehighestclassification; ambiguousloweringretains highestperrule. No
flagdrop/loweringacceptednow.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g001 record](records/review-through-00de0b6c/groups/g001.json). Record ID: `f2`.
