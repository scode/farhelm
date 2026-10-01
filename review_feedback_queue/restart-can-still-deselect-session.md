# Restarting a session can still deselect it

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

Restarting a session (especially a running one) can still make the main pane go empty or jump to a different session,
the same symptom #1310 set out to fix.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F8 / COR-RESTART-DESELECT`, tagged **possible**. Anchor and title:
`crates/farhelm-supervisor/src/service/core.rs:10358` — restarting a session can still deselect it in the browser; #1310
removed only one trigger.

While a session restarts, the supervisor removes it from its in-memory session table until the relaunched agent is
published again (`crates/farhelm-supervisor/src/service/core.rs:10358`). Any session listing served during that gap
simply omits the session, and the comment at `:10340-10357` accepts this because the window is "a couple of tmux round
trips". The helm caches whatever complete listing it receives and signals clients that the fleet changed. In the
browser, when a complete (unfiltered, untruncated) listing arrives without the currently selected session, the list
treats the session as deleted from another client and deselects it (`crates/farhelm-ui/src/list/view.rs:1255-1260`);
auto-select may then jump to a different session. When the session reappears a moment later, nothing restores the
selection.

PR #1310 removed the "sessions changed" hint the restart itself sent at the start of that window, but other listings can
still land inside it. The helm refreshes every host every 3 seconds regardless (`REFRESH_INTERVAL`,
`crates/farhelm-helm/src/manager.rs:160`). When restarting a running session, the restart first stops the agent
(`stop_live_agent`, `core.rs:10139`), which records stop-requested and stop-completed outcomes; each of those triggers a
hint (`core.rs:14298`), and hints are coalesced and sent at most every 200 ms
(`crates/farhelm-supervisor/src/service/hints.rs:62`, `:157`), so the stop-completed hint can go out after the session
has been taken off the table. Any other session on that host changing status does the same. Restarts started from
another client, or by an agent via `farhelm agent restart`, also lack the page-level lock that holds auto-select back in
the restarting client.

The result is the symptom #1310 set out to fix, now intermittent: the user restarts a session and the main pane empties
or switches to another session, most often when restarting a running one. Two reviewers agreed. It is "possible" because
the real length of the window compared with these triggers was not measured. The suggested fixes are either to keep the
session listed for the whole window (for example, a "relaunching" set the listing code reads alongside the session
table, reported with the pre-restart details and a launching-style status, while attach and teardown keep using the
table as today), or to make the UI deselect only on positive evidence of deletion (an actual deletion notice, or absence
from two consecutive complete listings). Add a browser test that forces a refresh into the window.
