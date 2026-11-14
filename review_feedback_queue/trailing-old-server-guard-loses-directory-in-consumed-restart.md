# Restart failure deletes the tmux socket before the explicit server guard runs

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: if an old-terminal test fails while restarting its supervisor, it can leave a private tmux server
running after deleting the directory required to stop it. This overturns c3's original proposed drop: explicit guard
presence is insufficient when directory ownership moves into an inner future.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g144/c3`, `g146/c4`.

### g144/c3

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 10 (finding, open):

Reviewed commit: dbfc69751215dbf15e1d31d5bed73c3754c4ebc4, with the preceding assigned harness ownership change at
647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR: if an old-terminal test fails while restarting its supervisor, it can
leave a private tmux server running after deleting the directory required to stop it. This overturns c3's original
proposed drop: explicit guard presence is insufficient when directory ownership moves into an inner future. Details:
main.rs:111 enables old_terminals. old_terminals.rs:266 and536 construct OldServer after the original Harness; its
fields at27-30 hold paths and TmuxServerGuard, not the TestDir. At332-334 and572-574 the live-server tests consume h
through restart_supervisor while the server guard stays in the caller. harness.rs:1130-1136 moves TestDir into that
restart future and awaits retire_supervisor; the latter has an explicit ten-second assertion at1100-1104 before the new
supervisor is built. If that assertion fails, unwinding drops the inner future's TestDir before the outer OldServer
guard. TestDir::drop at farhelm-teststate/src/lib.rs:794-805 stops only terminal hosts, and removes the directory when
those stops succeed. TmuxServerGuard stores only socket/executable (tmux/guard.rs:24-27), so its eventual Drop at75-96
acquires shutdown authority afresh from the now-deleted path. shutdown_tmux_server at tmux.rs:242-244 returns refused
when validate_socket cannot read the missing parent (368-369); refused at290-297 performs no protocol attempt and has no
fallback. Thus guard cleanup cannot reach the surviving server, and no directory is left for the dead-run sweep.
OldServer's config explicitly keeps dead panes (old_terminals.rs:42, 'set -s exit-empty on\nsetw -g remain-on-exit
on\n'), so the scoped-agent test already has a retained dead pane and the mixed-tab test's pane remains retained even if
its job cleanup runs. PidKillGuard only kills its recorded job, and MarkerCleanupGuard only exact session-marked
processes (harness.rs:3265-3274,3395-3404); the separately started tmux server is not that job and receives no session
marker (window construction applies -e only to panes). The no-server fixture already kills its server before restart and
is not a live-server route. Contract: tmux/guard.rs:17-20 requires the guard to stop the server 'before its state
directory dies' and explains drop order; these new callers violate that lifetime on failure. Suggested bucket: other,
because established impact is orphaned maintainer-test resources, not catastrophic tooling or shipped user-work loss.
Confidence: confirmed source and failure-path analysis; not behavior observed during execution. proposed_drop: false.
possible_cover: none; forbidden coverage sources not searched. Could-be-highest: no established security, user-work-loss
or catastrophic maintainer-tooling route. Caveats: trigger requires a restart failure before startup ends the old
server, such as the explicit retirement timeout, plus successful host cleanup/directory removal; successful tests do not
leak this way. No claim is made that the timeout happened in this review. Fix direction: carry the explicit server guard
and directory through one owner with guard-before-directory destruction, including failed/cancelled restart futures, or
retain directory ownership outside the consumed harness until server cleanup. All readily checkable premises settled; no
open lead remains for c3.

Returned proposal (claims-092, event 10): **retain-verified**, suggested bucket **other**.

The explicit server guard stays in the test caller while restart owns the directory. A restart failure before successor
construction can delete the socket directory before that guard acquires shutdown authority. This is a conditional
failure-path defect established by source, not an observed orphan.

Original stronger consequence: Removing generic guards could orphan a private fixture tmux server. The initial no-defect
proposal was explicitly withdrawn; latest evidence establishes the narrower consumed-restart lifetime route. No shipped
user-work-loss or catastrophic-tooling route was claimed.

Unresolved premise or limitation: No source premise remains for the conditional route. Actual retirement failure and
successful host cleanup/directory deletion were not observed; the claim does not assert frequency or a reproduced leak.

Proposed basis: Retain a new other-priority finding for the conditional fixture resource leak. Explain the failure
prerequisite and preserve the initial proposed-drop reversal.

Existing parent disposition (event 10): verified. Parent explicitly accepted named retain-verified proposal from
claims-092 after inspecting it. The explicit server guard stays in the test caller while restart owns the directory. A
restart failure before successor construction can delete the socket directory before that guard acquires shutdown
authority. This is a conditional failure-path defect established by source, not an observed orphan. Retain a new
other-priority finding for the conditional fixture resource leak. Explain the failure prerequisite and preserve the
initial proposed-drop reversal. Checked source/results:
["dbfc69751215dbf15e1d31d5bed73c3754c4ebc4:old_terminals.rs:27-49 stores paths and a guard, not TestDir; :42 retains
dead panes. :266,332-334 and :536,572-574 leave live-server ownership outside the consumed restart future.","Same
revision harness.rs:1100-1104 can panic while retiring the predecessor; :1130-1137 moves state into the future before
retirement and builds the successor only afterwards. Failure drops inner TestDir before outer OldServer.","Same revision
farhelm-teststate/src/lib.rs:794-806 removes the directory if host cleanup succeeds, without stopping tmux.
tmux/guard.rs:24-27 retains no socket peer; :79-96 reacquires authority at Drop. tmux.rs:242-244,290-297,367-368 refuses
a missing parent and performs neither protocol shutdown nor fallback.","The original line-3 proposed drop is overturned
by the later owner/restart analysis. The no-server fixture is not the live-server route; successful restart kills the
server before this ordering matters. No tests or reproductions were run."] Current coverage/results: ["SPEC.md Terminal
experience and SPEC_impl.md Ending the old private tmux server/Testing do not accept fixture directory removal before
its guard. The guard's reviewed doc at :17-23 explicitly requires shutdown before the directory dies.","TODO.md Planned
is empty; BUGS.md and active TRIAGE_OUTCOMES.md have no matching restart-owned-directory/old-server claim.","INDEX.md
and queue searches found related terminal-host cleanup records, including
mac-process-lookup-truncates-live-host-command.md and blocking-test-cleanup-prevents-async-listener-release.md, but
their triggers and resource owners differ. No exact alias found.","plans/queue/tmux-drop-from-tooling.md keeps the
old-server tests and dead-run sweep; plans/REPORTS.md records it landed. The later removal TODO is Mid term, not
Planned; no unlanded plan fixes this owner lifetime. FILTER.md's rare diagnostics/safe-retry boundary excludes the
persistent orphan-resource consequence."] Exact cited cover: [] Original stronger consequence and missing premise remain
preserved: "Removing generic guards could orphan a private fixture tmux server. The initial no-defect proposal was
explicitly withdrawn; latest evidence establishes the narrower consumed-restart lifetime route. No shipped
user-work-loss or catastrophic-tooling route was claimed."; "No source premise remains for the conditional route. Actual
retirement failure and successful host cleanup/directory deletion were not observed; the claim does not assert frequency
or a reproduced leak.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g144 record](records/review-through-00de0b6c/groups/g144.json). Record ID: `c3`.

### g146/c4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 11 (finding, open):

Reviewed assigned commit: 303a1264b7bd791e8f6f6de2c7bb6136c08e8bab; this cross-group caller route is introduced earlier
within the pinned range, by dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 old-terminal callers after the consumed-restart
ownership change in 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513 (g144 owns that provenance). TLDR: a failed supervisor
restart in an old-terminal test can delete the socket directory before the private tmux guard runs, leaving its server
alive and unreachable by normal cleanup. This is source-confirmed overlap with incoming g144/c3, not a distinct new
discovery. Details: at dbfc6975 old_terminals.rs:266 creates OldServer after h, :332-334 consumes h into
restart_supervisor while server remains outside. OldServer:27-30 holds only paths/guard; :42 keeps dead panes, and
:90-99 gives session markers only to pane launches, not the separate server. Harness:1130-1136 moves TestDir into the
future; retire_supervisor:1100-1104 can assert on a ten-second reference-drain timeout before new startup can end the
old server. Unwinding destroys inner TestDir first. TestDir::drop at teststate/lib.rs:794-806 only calls end_hosts and
then removes its directory if hosts were confirmed ended. The outer path-only guard then runs. Assigned-revision
guard.rs:25-27 retains no peer or directory descriptor; :65-69 acquires diagnostic/shutdown authority at Drop.
tmux.rs:266-268 calls validate_socket; :391-392 refuses missing parent metadata; :314-320 consequently attempts neither
protocol nor fallback. The guard-before-directory contract at guard.rs:17-22 is violated on this caller failure path.
Assigned-revision harness.rs:974-981 retains this consumed owner/failure sequence. Fix direction: keep directory and
explicit server guard in one lifetime through failure/cancellation, or retain the directory outside the consumed restart
until server cleanup. Suggested bucket: other (orphaned maintainer-test server/resource, not catastrophic tooling).
Confidence: confirmed conditional source route, not observed execution. Could-be-highest: no established
user-owned-work, security or catastrophic tooling route. proposed_drop for the directory-lifetime finding: false.
Original consequence and proposed-drop basis retained: removed with_environment/diagnostic_text APIs were suspected of
breaking fixture cleanup/diagnostics; their absence remains refuted by remaining matching launch environments and Drop
tracing callers, but that narrow refutation never disposes of the independent consumed-future ownership failure.
possible_cover: g144/c3 is the exact already reported route; parent should consolidate, not count twice. Caveats:
requires restart failure before startup kills the old server plus successful host cleanup/directory removal; no timeout
occurrence is claimed, and successful restarts need not leak. No parent drop/lowering accepted. Source premises checked
above; no unresolved lead for c4. No test, reproduction, build or live-install action.

Returned proposal (claims-093, event 11): **retain-verified**, suggested bucket **other**.

A failed consumed restart can remove the private socket directory before the caller's path-only server guard drops. The
guard then cannot acquire authority, so its normal shutdown cannot reach a server that remains alive. This is a
conditional source conclusion, confined to maintainer test resources, not an observed timeout or user-session loss.

Original stronger consequence: The original candidate suspected removed explicit-environment/diagnostic-text APIs could
break private-server fixture cleanup or lose diagnostics. Those particular API regressions remain source-refuted. The
later, distinct consumed-future ownership route establishes potential orphaning of the test's server; it was not
withdrawn or lowered from an established highest consequence.

Unresolved premise or limitation: No missing source premise for the conditional failure. Actual reference-drain
timeout/cancellation occurrence and leak duration were not observed. The route requires failure before successor startup
ends the old server, successful host cleanup/directory removal, and the private server still alive. No security,
user-owned-work or catastrophic tooling consequence is established.

Proposed basis: Retain once as verified conditional maintainer-tooling cleanup failure, other priority. The supplied
history explicitly names g144/c3 as the same route; that record itself was not an input to this batch, so the parent
must reconcile the alias rather than infer that this consultation authorizes a duplicate drop. No existing queue item
fully covers it.

Existing parent disposition (event 11): duplicate. Parent read full093 and092: same consumed restart future owns/drops
TestDir on retirement failure before outer OldServer path-only guard drops; missing socket parent refuses guard
authority and leaves private server alive. Canonical144c3event10 retained with same source/conditional cleanup
consequence. Preserve additional 303a guard/API refutations, direct-source failure prerequisites, tip persistence and no
runtime/user-work-loss claims. One current tooling resource-lifetime item.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g146 record](records/review-through-00de0b6c/groups/g146.json). Record ID: `c4`.
