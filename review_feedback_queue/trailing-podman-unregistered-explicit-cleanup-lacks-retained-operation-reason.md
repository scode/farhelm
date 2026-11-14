# Closing Containers loses an unregistered cleanup failure's reason

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: an unregistered container cleanup failure disappears when Settings closes. Reopening
shows the leftover and a fresh Delete button without the prior failure reason; closing before the request finishes can
discard its only visible failure report entirely. Source: containers_settings.rs:46 stores action_error only in the
component. The admitted delete survives closing (:165-167), but :168-170 returns before handling its result when
unmounted; while mounted :174 records the failure only in that local signal. Neither reads nor row rendering restore
operation failures. ContainerLeftoverView (proto/environment.rs:157-166) has id/name/place/role/host and no cleanup
result; podman.rs:828-834 constructs that bare view from ownership discovery. For the concrete unregistered route
(row.host=None), api/environments.rs:80-89 uses DELETE /api/container-orphans. Its handler environment/podman.rs:876-907
runs owned cleanup but returns failure only as a 502 body, without retaining a failed operation. Provider
delete_leftover at :725-763 revalidates identity/ownership then calls remove_container; its errors do not create a host
record or another diagnostic owner. A later successful discovery therefore recovers the remaining resource, not its
previous failure reason. Contract: SPEC.md:4812-4814 requires a Containers leftovers list "with confirmed Delete and
retryable failure, following the sandbox-leftovers pattern." That referenced pattern explicitly says at :524-526:
"Unfinished cleanup remains in the list with its error and **Retry cleanup**, even after closing and reopening Settings;
runni

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g077/f9`.

### g077/f9

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 21 (finding, open):

Reviewed commit: 350a4b33b6d4476f4a78e43cea124bc2b828f462. Discovery pass 2. TLDR: an unregistered container cleanup
failure disappears when Settings closes. Reopening shows the leftover and a fresh Delete button without the prior
failure reason; closing before the request finishes can discard its only visible failure report entirely. Source:
containers_settings.rs:46 stores action_error only in the component. The admitted delete survives closing (:165-167),
but :168-170 returns before handling its result when unmounted; while mounted :174 records the failure only in that
local signal. Neither reads nor row rendering restore operation failures. ContainerLeftoverView
(proto/environment.rs:157-166) has id/name/place/role/host and no cleanup result; podman.rs:828-834 constructs that bare
view from ownership discovery. For the concrete unregistered route (row.host=None), api/environments.rs:80-89 uses
DELETE /api/container-orphans. Its handler environment/podman.rs:876-907 runs owned cleanup but returns failure only as
a 502 body, without retaining a failed operation. Provider delete_leftover at :725-763 revalidates identity/ownership
then calls remove_container; its errors do not create a host record or another diagnostic owner. A later successful
discovery therefore recovers the remaining resource, not its previous failure reason. Contract: SPEC.md:4812-4814
requires a Containers leftovers list "with confirmed Delete and retryable failure, following the sandbox-leftovers
pattern." That referenced pattern explicitly says at :524-526: "Unfinished cleanup remains in the list with its error
and **Retry cleanup**, even after closing and reopening Settings; running cleanup shows **Cleaning up…**. This operation
state lasts until cleanup succeeds or the helm restarts." SPEC_impl.md:6569-6570 independently says "Failed explicit
cleanup keeps its resource and reason visible for retry." The missing retention is confirmed from source; no runtime
failure was reproduced. Scope/caveats: this finding uses unregistered leftovers, which have no Hosts row carrying a
separate failure; it does not claim all row-backed failures lack diagnostics. The resource remains discoverable and the
user can retry via Delete, so no permanent lockout, work loss, automatic cleanup, or missing-resource claim. Switching
Settings sections alone keeps the component mounted and preserves its local error; closing/reopening is the failing
boundary. Suggested bucket: other, lost cleanup diagnostic/retry context. Could-be-highest:no established security or
user-work-loss route. Confidence: confirmed source/contract analysis. proposed_drop:false. possible_cover:none in
supplied neighbour projection; distinct from g060/l1 recipe-status request retry and g061/l5 completion invalidation.
Fix direction: retain in-flight/failed explicit unregistered cleanup state outside the Settings opening and project it
into the list, as the referenced sandbox pattern does; do not add automatic retry or startup cleanup. No runtime tests,
reproductions or live-install actions.

Returned proposal (claims-050, event 21): **retain-verified**, suggested bucket **other**.

For an unregistered leftover, failed cleanup is reported only to the initiating Settings component. Closing it loses
that error, including a failure arriving after close, contrary to the retained cleanup-state contract. The resource
remains discoverable and can still be deleted again.

Original stronger consequence: The final record extends the initial after-reopen loss to closing before completion,
which can discard the only visible failure report. It continues to state that the resource remains retryable and no user
work is lost.

Unresolved premise or limitation: No runtime cleanup failure or frequency observation. No missing-resource, permanent
lockout, automatic cleanup or work-loss route is established. Row-backed resources may have an independent Hosts
diagnostic and are outside the concrete retained scope.

Proposed basis: Retain only unregistered explicit cleanup state/reason retention across Settings openings, including
in-flight completion after close.

Existing parent disposition (event 21): verified. Parent explicitly accepted named retain-verified proposal from
claims-050 after inspecting it. For an unregistered leftover, failed cleanup is reported only to the initiating Settings
component. Closing it loses that error, including a failure arriving after close, contrary to the retained cleanup-state
contract. The resource remains discoverable and can still be deleted again. Retain only unregistered explicit cleanup
state/reason retention across Settings openings, including in-flight completion after close. Checked source/results:
["350a4b33b6d4476f4a78e43cea124bc2b828f462:crates/farhelm-ui/src/containers_settings.rs:46 keeps action_error locally;
:165-174 lets deletion survive but drops result handling after unmount. Section switching preserves the mounted
component; closing/reopening does not.","Same revision:api/environments.rs:80-95 sends host=None rows to DELETE
/api/container-orphans. crates/farhelm-helm/src/environment/podman.rs:725-765 revalidates ownership/references and
removes the resource; :876-907 owns the attempt but returns its error only in the HTTP reply. No failure-operation
record is created there.","Same revision:farhelm-proto/src/environment.rs:160-166 exposes only id/name/place/role/host;
environment/podman.rs:827-833 constructs that view without prior cleanup results. A successful later discovery restores
the resource, not the lost reason.","Same revision:SPEC.md:4812-4814 adopts the sandbox-leftover pattern, whose cleanup
state survives Settings closure; SPEC_impl.md:6569-6570 explicitly retains resource and reason for retry. No failure was
reproduced. The claim is conditional on cleanup returning an error, not on its measured frequency."] Current
coverage/results: ["Current SPEC.md:528-539 retains the explicit error/Retry cleanup state across closure and Container
sessions:4888-4890 adopts that pattern. SPEC_impl.md:6089-6091 requires visible failed cleanup; :6810-6829 describes the
separate sandbox implementation. These require the missing behavior, not acceptance of its omission.","TODO.md Planned
empty. Searched unlanded plans/queue, BUGS.md, INDEX.md and active TRIAGE_OUTCOMES.md for container cleanup error
retention/Settings close/retry. No exact coverage; row-backed host failures, sandbox orphan discovery overlays and
recipe-status issues are separate scopes. Relevant Podman plan is already landed.","FILTER.md diagnostic-loss match is
not established: every failed unregistered cleanup loses its report on ordinary closure, without a proved rare timing
trigger. Session-operation status filtering is a different resource/operation scope. Retain rather than infer rarity."]
Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "The final record extends the
initial after-reopen loss to closing before completion, which can discard the only visible failure report. It continues
to state that the resource remains retryable and no user work is lost."; "No runtime cleanup failure or frequency
observation. No missing-resource, permanent lockout, automatic cleanup or work-loss route is established. Row-backed
resources may have an independent Hosts diagnostic and are outside the concrete retained scope.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g077 record](records/review-through-00de0b6c/groups/g077.json). Record ID: `f9`.
