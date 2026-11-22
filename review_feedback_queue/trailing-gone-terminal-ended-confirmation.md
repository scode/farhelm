# Stop or Delete can exceed an ended-session confirmation

Reviewed commit: 063203912059cb23101d9a458d8821f02f1bc18f

## TLDR

When a lost terminal is shown as ended, Stop or Delete can terminate a main program the confirmation did not say it
would stop.

## Details

**highest.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/handlers.rs:879`.

tip hosts/terminal.rs:823-835,843-854 and status.rs:195-204,244-247 turn a held Gone host into an ended display without
checking the child. The ordinary session header can offer Stop (GPUI session_pane/header.rs:795-815,1189).
Session_list/stop.rs:65-75,93-105,185-195,219-235 freezes the ended guard from the same row that asks only about its
tabs, and helm sessions.rs:3092-3105,3138-3144 forwards it. Supervisor handlers.rs:839-912 bypasses the guard when there
is a retained Gone handle and no probe PID, and :1120-1132 sends the scoped launch to the reaper. The host can exit
without waiting for the child (terminal-host/SPEC.md:149-155). The host is outside the program scope
(handlers.rs:894-898). For a surviving main child in a still-live recorded scope, sweep.rs:1609-1615,1868-1916 and
scope.rs:1196-1200,1241-1251 send TERM and escalate through that scope regardless of the missing root. This route needs
no unreadable or missing markers. SPEC.md:2203-2224 and SPEC_impl.md:4691-4696 require a refusal before signalling if
the child is still alive or its exit cannot be confirmed. The background cleanup exception does not authorize killing
the main program. Refuse the guarded request on a live or unresolved retained pin before classification or cleanup.
Existing Stop guard test at handlers.rs:4116-4159 covers a missing handle plus Launching only. Bucket: highest,
potential termination of the user's process beyond explicit consent through an established shipped control. Confidence:
source-supported conditional route. No runtime observation or separate data-loss consequence. proposed_drop: none. Can
combine with l10 under the same consent defect if both handlers and their different cleanup routes remain explicit.
Possible coverage: l10, recovery r6, recovery_contract.

Additional source record at `063203912059cb23101d9a458d8821f02f1bc18f`,
`crates/farhelm-supervisor/src/service/core.rs:13468`.

tip core.rs:13439-13487 admits Gone through the main terminal's probe and then checks recorded hosts.
session_hosts.rs:238-260 omits the held non-placeholder socket from rediscovery; :587-634 does not check that main
child. After guard acceptance, teardown.rs:320-345,530-539 revalidates the retained pin and seeds WholeSession cleanup
even without a scope or readable marker. hosts/terminal.rs:759-789 documents a surviving pinned child and
:4037-4047,4133-4148 preserves the pin through the Gone transition. Terminal-host/SPEC.md:149-155 says the host does not
wait for the child at exit. Tip status.rs:195-204,244-247 presents Gone as exited for a previously running row; GPUI
session_list/delete.rs:81-99,317-344 sends NothingAlive or AgentEnded and helm sessions.rs:3976-3990 derives that guard.
SPEC.md:2203-2224 requires refusing more alive than the user agreed to stop. The background exception does not cover the
main program. A process-table read error already refuses in prepare_teardown. The proof here is the positive pin
verification after the guard. Existing core/tests/host_terminals.rs:302-359 covers an unreadable tab changing during the
guard, not the retained main pin. This ordering is present at exact parent fc2b0b46 (core.rs:13433-13468,
teardown.rs:320-345). Verify the held main child before guarded Delete can authorize teardown. Bucket: highest for a
source-supported route terminating the user's main process beyond confirmation. Conditions: ordinary session, retained
Gone runtime, verified surviving main child and guard from the ended presentation. No runtime observation or separate
lost-data claim. Confidence: source-supported authority and cleanup mismatch. proposed_drop: none. Can combine with l9
under the same consent defect if both handlers and both termination routes remain explicit. Possible coverage: l9 and
recovery_contract. R6 independently supplies the held-child premise but concerns omitted Restart cleanup.

Additional source record at `df6199142d6495a95b2d008400ba2686efb775c6`,
`crates/farhelm/tests/e2e/host_terminals.rs:552`.

host_terminals.rs:552-595 tests AgentEnded with a live held host and only checks Gone after unconditional Delete;
:705-750 explicitly drops the held runtime while its recorded host stays alive; :868-905 observes an unreadable
additional generation while the main program has stopped. terminal_tabs.rs:1000-1127 checks ended-agent consent against
a reachable restarted host and checks success after a real exit. session_lifecycle.rs:3861-3878 checks external host
loss under unconditional Delete, without an independent main-child witness. Its wait helper, harness.rs:1969-1997,
accepts Gone, Unavailable, unpublished, or closed status as its wake condition. launch_sentinel_error_status.rs:515-549
and :599-619 do observe exact Gone before listing or reconciliation, but issue no guarded lifecycle request and do not
observe a surviving or unresolved main child. SPEC.md:2203-2224 requires refusing an ended-only action when the main
program is alive or unconfirmed. The missing E2E assertion needs three independent facts before dispatch: a retained
main runtime in Gone, its pinned child still live or unconfirmable, and ended-only request consent. The expected result
is the guard's refusal, with session, main child when live, and any tab left intact. Bucket: other for a distinct
assertion gap. Confidence: confirmed by pinned sources and enumerated control call sites. No test was run.
proposed_drop: retain this as coverage evidence merged with Stop/l9 and Stop/l10. Possible coverage: none for the exact
conjunction. The listed tests cover the inputs separately. Adjacent in-process guard assertions also separate the
premises: service/handlers.rs:4116-4159 explicitly seeds terminal=None plus Launching for Stop.
Service/core/tests/host_terminals.rs:302-359 uses an unavailable tab and no main terminal for Delete. The E2E call-site
search found no additional raw StopSession or DeleteSession dispatch carrying the flags. The later in-range handler and
host-terminal-unit changes do not add this witness. Disposition for the next pass: retain the missing assertion as
coverage evidence with Stop/l9 and Stop/l10. Do not count it as a distinct new issue. Highest potential: the linked
product issues carry that consequence. The coverage note does not add an independent consequence.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `stop/l9`, `stop/l10`,
`lifecycle_e2e/l9`.
