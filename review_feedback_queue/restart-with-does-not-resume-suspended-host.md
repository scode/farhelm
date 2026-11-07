# Restart with does not resume suspended host

Reviewed commit: 8a7f66ede90651d47d1facc1e877ce81f842af1a

## TLDR

Restart with on a suspended sandbox does not perform the resume step, so the requested restart fails instead of
continuing.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: other.

Source location:
`crates/farhelm-ui/src/list/create_form.rs:3735-3742,5263-5424 at 8a7f66ede90651d47d1facc1e877ce81f842af1a`.

Source assessment (read with the later corrections below) (review record g057/d1, event 12):

Reviewed commit: 8a7f66ede90651d47d1facc1e877ce81f842af1a. TLDR: If a Tensorlake host suspends while its Restart with
editor is open, the Restart action becomes available but fails because that path never resumes the host. New shared-form
availability treats suspended hosts as usable, while resume-before-dispatch was added only to the ordinary
create/replace path. Mechanism: assigned change at create_form.rs:3735-3742 and 5129-5134 permits host.suspended. A
source session with usable recorded identity still passes restart_host_confirmed (:3024-3035; matching_host_option in
list/shared.rs:554-568 compares identity/mismatch, not connection phase). Restart mode holds the existing destination,
so the Github-only resume branch :5138-5154 does not apply. LauncherMode::Restart at :5263-5424 calls
api::restart_session (:5379-5386) then returns (:5424), never reaching the ordinary existing-folder resume at
:5593-5618. For an agent launch with stored resume command, the live bridge does not close this route:
session_view.rs:98-114 returns no restart-with reason independent of stale phase, and admit_restart :1365-1387 checks
launch kind then admits. The view may remain mounted while the host is unavailable; this route needs no stale callback
or event race, just an already-open editor and supported external/idle suspension. The API helper at api.rs:3495-3513
only posts /sessions/{id}/restart. Backend sessions.rs:2811-2832 routes with route_session, which :819-846 refuses a
nonconnected host immediately with Conflict and does not resume/queue it. No session mutation/data loss follows.
Temporal check: pinned tip 8a8c9579e04299410652f0e4681975a7198dfc7b retains shared HostOption::available_for_launch
(shared.rs:398-399 includes self.suspended), uses it at create_form.rs:3841-3846 and 5235-5239, and restart dispatch
:5510-5517 returns at :5555 before ordinary resume. Governing contracts read: SPEC.md Topology :276-284 promises resume
before launching on suspended hosts; Lifecycle operations :1528-1552 gives Restart with the ordinary launcher
validation/availability rules and leaves refused edits visible; SPEC_impl.md GUI Restart with :759-777 documents its
separate submit routing and bridge. At minimum the newly enabled action is deterministically futile for the known
suspended phase, even if automatic host resume is intentionally restricted to fresh launches. Fix either perform
explicit resume and revalidate source/installation before restart dispatch, or keep Restart mode unavailable until
connected rather than sharing suspended-create readiness. Source-derived regression: open Restart with for a resumable
agent session while connected, change a setting, have its host report Suspended, then press Restart; verify resume
occurs before restart or button is disabled with an actionable resume-first explanation. Suggested bucket other (narrow
convenience/availability failure, recoverable by explicit Resume; no established high/highest impact). Confidence
confirmed source route; no dynamic/cloud/runtime test run. proposed_drop none. possible_cover unknown; parent owns
coverage, not found in supplied known records. Caveat: no claim that plain Restart should auto-resume, or that an old
callback survives a render. New verified discovery ID d1; producer is the supported host suspension while a normal
editor remains open.

Later qualification (review record g057/d1, event 14):

Supplement to complete d1 record above; reviewed full hash 8a7f66ede90651d47d1facc1e877ce81f842af1a, persists through
8a8c9579e04299410652f0e4681975a7198dfc7b. TLDR remains: Restart with becomes enabled on a suspended host but does not
resume it and predictably fails. Additional producer checks: create_form.rs:1127-1139 target_matches_selection compares
selected id and installation incarnation, not phase; list/view.rs:1386-1395 continues constructing CreateTarget from
host_options for that effective host without a connected-only filter. Thus the earlier target-match guard does not
secretly prevent the suspended route once the ordinary host refresh settles. create_form.rs:2992-2998 reads the
still-mounted RestartBridge source without closing the editor on stale status. Pinned-tip list/view.rs:2968-3021 sets
restart_bridge and show_create on the header request; :4838-4849 and :4882-4890 clear on successful restart/cancel, not
host suspension. Existing evidence and exact dispatch/refusal source remain in prior d1 event, including restart return
before ordinary resume, backend nonconnected conflict, scope-specific SPEC and suggested fix/regression. Suggested
bucket other, confirmed by source; no runtime reproduction, no high/highest harm claimed. proposed_drop none;
possible_cover unknown, parent handles coverage. This resolves d1 lead as a new verified source finding. Discovery pass
1 complete; no separate unresolved premise or lead remains for d1. c1-c3 unchanged and c4 separately records the one new
rejected presentation candidate.

Recording coverage: Pending one-press-suspended-checkout plan affects fresh GitHub New/template launch, not Restart
with. No exact cause removal.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g057/d1`; supporting records
`g058/g058-c7`. The retained private log preserves earlier wording and its corrections. This record does not claim
runtime reproduction.
