# Rename stays enabled for retained sessions while their environment is being deleted

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: A retained session row can show that its sandbox or container is being deleted while
still opening an enabled Rename editor. The editor can submit a request that the helm refuses because that environment
has no routable connection. The established issue is the offered unavailable editor, not a successful mutation.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g037/f1`.

### g037/f1

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 11 (finding, open):

Reviewed commit: 2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa

TLDR: A retained session row can show that its sandbox or container is being deleted while still opening an enabled
Rename editor. The editor can submit a request that the helm refuses because that environment has no routable
connection. The established issue is the offered unavailable editor, not a successful mutation.

Details: Synthesis confirms the full availability/admission route. row_control_visibility at row.rs:144-150 sets
rename=true unconditionally for ordinary non-retired rows. The new environment_deleting flag at :1179-1184 guards
Clone/Delete, while frozen covers Stop/Restart/Replace; Rename at :2671-2716 checks only busy (:2684/:2707).
list/view.rs:6312-6317 derives busy from local pending/held/questions. on_rename_start at :3850-3888 checks retired_row,
local busy/questions and an existing editor, but never host phase. retired_row at :1249-1255 checks only
retired-container compatibility. on_rename_submit at :3909-3945 checks the editor generation/unavailable flag and
begin_row_op; begin_row_operation at :868-876 checks only local questions/claims/pending before reserving.
listed_rename_unavailable is reconciled only against authoritative source absence at :2250-2262, not the Deleting host
phase. Thus a row still displayed with no local request/confirmation can open and submit Rename.

The failed sealed-destruction premise is preserved and bounded precisely: manager.rs:2610-2619 notes a seal
independently of any GUI session claim; sessions/sandbox.rs:290-304 records an attempt and failure; manager.rs:2639-2679
retains Failed(reason) as HostState::Deleting, and :2210-2228 exposes no client whatever the actor's connection does.
finish_seal runs in the helm-owned finisher (:339-360), not a page's SessionOps. hosts.rs:1537-1545 returns on provider
destruction failure before deleting the registered host/cache at :1550-1554. While a cached session row remains
displayed after that failure, no failed sealed-host reconnection can make its Rename request routable. A later
successful supervisor refresh may show the final session already gone and remove its row; this finding does not assert
that a failed destruction always leaves a session row forever. The same omission is visible while a host-level
whole-environment destruction holds Deleting at hosts.rs:1513-1534 and a cached session row remains, without a
session-local GUI claim.

Backend boundary: manager.status removes the client for every seal record (:2220-2224), and sessions::route_session at
:905-926 refuses status.client=None with Conflict. An attempted rename is therefore refused before reaching the sealed
supervisor. This prevents mutation and limits the effect to an invalid available control, an editor and its refusal.

Contract: SPEC.md:1521-1523 explicitly covers both active and failed destruction: "While that runs, and if it fails,
Hosts and the sidebar attention area show the sandbox as being deleted, with the reason and **Retry**, and its sessions
show as being deleted with their actions unavailable." Container sessions at :4867 inherits that rule.
SPEC_impl.md:6639-6648 says seal state must preserve Deleting/no-client across holds/releases/reconnects and that
Deleting sessions' actions are unavailable. No Rename/metadata exception is stated for this phase. A host-phase guard in
Rename presentation and local admission should match the other unavailable lifecycle controls.

Suggested bucket: other, preserving the original offered-editor/refusal consequence. Confidence: confirmed source and
contract analysis for retained displayed rows; no execution observed. Could-be-highest: no established
security/loss-of-work route. proposed_drop: false. possible_cover: none known. Material caveats: a local Delete claim
can independently disable Rename while it is pending, so the failure/helm-finisher/other-client or host-level path
matters; disappearance of the cached row ends the opportunity. No mutation, data loss, permanent retained row or runtime
reproduction is claimed.

Returned proposal (claims-022, event 11): **retain-verified**, suggested bucket **other**.

A retained ordinary session row in Deleting can open and submit Rename because presentation and admission omit the host
phase. The helm refuses routing.

Original stronger consequence: The earliest finding called this a working Rename editor and error state; its explicit
caveat already excluded successful backend mutation. Latest wording correctly limits it to an offered editor and refused
request.

Unresolved premise or limitation: No missing premise stated in this returned proposal.

Proposed basis: Retain as other. A failed destruction can hold the phase while a row remains displayed, so the whole
claim is not a rare self-correcting display race. Do not make another-GUI-only consistency the trigger: the helm-owned
finisher/host-level deletion also supplies the phase.

Existing parent disposition (event 11): verified. Parent explicitly accepted named retain-verified proposal from
claims-022 after inspecting it. A retained ordinary session row in Deleting can open and submit Rename because
presentation and admission omit the host phase. The helm refuses routing. Retain as other. A failed destruction can hold
the phase while a row remains displayed, so the whole claim is not a rare self-correcting display race. Do not make
another-GUI-only consistency the trigger: the helm-owned finisher/host-level deletion also supplies the phase. Checked
source/results: ["At 2ef10815f12ed719ab6c51c6e3ac6fcd3f8bc1fa, list/row.rs:144-150 enables rename for ordinary rows;
:1179-1184 computes environment_deleting, but Rename :2671-2716 checks busy alone.","list/view.rs:6312-6317 derives busy
from page-owned pending/claims/questions. :3850-3888 opens Rename without a host-phase guard; :3909-3945 submits through
begin_row_op without one.","list/view.rs:2248-2262 retires questions and updates rename unavailability from
authoritative source presence, not host Deleting.","manager.rs:2210-2228 withholds every sealed host's client;
sessions.rs:905-926 refuses a missing client with Conflict. This establishes refusal, not successful rename or lost
work.","2ef10815 is in range. Source-conditional on the row remaining displayed; no runtime observation."] Current
coverage/results: ["Recording-time main 24bc058066fd57848e788b465719b0e6da2f98f7: TODO.md:25-26 Planned is
empty.","BUGS.md searched for actual Restart, Deleting, marking, Replace-progress and notification claims; its sole
SSH-helper install/update hang is unrelated.","plans/queue searched for retirement/stale/floor/success,
Deleting/Rename/question, marking/cleanup, ReplaceProgressGuard and OldTerminalEnded. Matching session-action-leftovers,
environment-agent-operations, tmux and nonblocking plans have landed per plans/REPORTS.md; none is an unlanded repair
covering these claims.","review_feedback_queue/INDEX.md read in bounded chunks. Matching
retained-restart-hides-reboot-card, suspended-sandbox-keeps-row-confirmations-enabled,
frozen-sandbox-retains-header-lifecycle-answers and bulk-sandbox-delete-drops-eligible-peer files read; their
trigger/consequence boundaries differ as noted per record.","TRIAGE_OUTCOMES.md searched for the actual
retirement/stale-success, Deleting/Rename/question, marking cleanup, progress-guard and notification claims; no matching
active disposition. Archive not read.","SPEC.md Sandbox sessions:1525-1528 and SPEC_impl.md:6148-6157 explicitly require
actions unavailable while deletion runs or has failed. Container sessions share that policy. No Rename metadata
exception found. Existing frozen/suspended lifecycle-answer queue items concern other controls and phases."] Exact cited
cover: [] Original stronger consequence and missing premise remain preserved: "The earliest finding called this a
working Rename editor and error state; its explicit caveat already excluded successful backend mutation. Latest wording
correctly limits it to an offered editor and refused request."; null.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g037 record](records/review-through-00de0b6c/groups/g037.json). Record ID: `f1`.
