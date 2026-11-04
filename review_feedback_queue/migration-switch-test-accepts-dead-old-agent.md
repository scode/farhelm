# Switch-on regression accepts a dead old agent as preserved

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The update regression can pass if the old running agent dies during supervisor stop/start, so it fails to test its
stated keep-working guarantee.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed by source, no
reproduction necessary to establish that the asserted observables permit a dead retained pane. No actual shipped
shutdown/agent-loss bug is claimed.

Source locations: crates/farhelm/tests/e2e/host_processes.rs:631-648.

host_processes.rs:574-589 states that old running sessions must keep working; :602 waits for READY before restart; :631
repeats wait_for_agent_ready afterwards, then :632-648 merely checks Bytes stream kind, reads no data and sends no
input. Same-commit harness.rs:1125-1143 captures 200 history lines and returns on the historical READY substring before
asking whether the pane is dead. Dead agents retain a viewable terminal by SPEC.md Terminal experience (2120-2129 at
pinned tip). Same-commit service/terminals.rs:918-943 resolves an agent by its retained terminal, without a liveness
guard; handlers.rs:1494-1542,1888-1940 opens tmux replay/input and replies Attached Bytes, without requiring a live
agent. Thus an exited retained tmux pane can satisfy every asserted old-session observable.

the update regression can pass if the old running agent dies during supervisor stop/start, so it fails to test its
stated keep-working guarantee. tip host_processes.rs:574-589 states that old running sessions must keep working; :602
observes startup READY before restart, while :631 repeats wait_for_agent_ready afterwards, and :632-648 merely checks
Bytes stream kind, consumes no data and sends no input. harness.rs:1125-1143 captures 200 history lines and returns on
historical READY before asking pane_dead. Later synthesis checked all remaining attach setup premises:
tmux/input.rs:23-79 only attaches a no-output control client to the session and drains its attach reply, with no
live-agent check or test input; tmux/stream.rs:271-375 attaches its output client, snapshots and cuts over; :1081-1155
reads mode/history/visible blocks without rejecting a dead agent. service/terminals.rs:918-943 resolves the retained
agent terminal, and handlers.rs:1888-1940 reports Attached Bytes. SPEC.md Terminal experience explicitly keeps
stopped/exited agent terminals viewable. Thus a dead retained pane with its startup history satisfies every old-session
assertion in this test.

Possible coverage and caveats: TRIAGE_OUTCOMES.md:10720-10738, shutdown-tests-prove-session-existence-more-strongly.md,
fixes three named supervisor_stop.rs variants and not this new default-switch test; :11014-11036 concerns a different
unit fixture. SPEC_impl.md:2004-2022 Turning terminal hosts on requires retaining existing terminals when the switch
changes and does not accept this oracle gap. TODO.md Planned is empty (:23-25). Searches of SPEC.md, SPEC_impl.md, TODO
Planned, all plans/queue, BUGS.md, queue INDEX/matching files and ledger found no exact trigger/consequence/scope
coverage. FILTER.md:41 excludes success-for-failure from the rare safe-failure filter; this false-pass finding is not
f1's rare false failure.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g190/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb;
introduced in 58094288259b0c29f74ea25936d4fe006c72128f. Tip persistence verified: git diff between those commits is
empty for host_processes.rs, harness.rs and fake_agent.rs.. This was a source-only review; runtime occurrence and
frequency were not measured.
