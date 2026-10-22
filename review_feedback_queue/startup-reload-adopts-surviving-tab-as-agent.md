# Pane-less recovery can adopt a surviving terminal tab as the session's agent

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

A session's agent window is gone but one of its terminal tabs is still open, perhaps running a dev server or a long
build. The user clicks Restart. If the supervisor is restarted or updated before the new agent window appears, or the
restart fails with an uncertain result, the supervisor can come back believing the user's tab is the agent. It then
shows the session as running with the tab as the agent terminal. From then on Stop kills that tab and everything running
in it, and Restart replaces it with the agent, without the confirmation closing a tab would ask for. A later Delete no
longer counts it as an open tab. While the restart is in flight, a conversation report from a program in the tab can
also be accepted as the session's own, so a later Resume reopens the wrong conversation. Whether the agent window can go
missing while a tab survives without someone using Farhelm's private tmux server directly is unsettled.

## Details

Found by the 2026-10-22 prioritized persistent review, groups `105-supervisor-core-7of15` (finding f2) and
`078-tmux-input-4of6` (finding f1), merged here because they describe the same defect. The 078 finding was first
proposed for a drop; an independent reviewer judged that drop ambiguous and asked for one merged item so triage settles
reachability.

Suggested bucket: highest (Stop and Restart kill a user's tab processes, a destructive action on the wrong target; a
wrong conversation can be resumed). 078 suggested highest only if the triggering shape arises without direct use of the
private tmux server.

Confidence: likely. Confirmed by reading (reviewers and recording): the marker skip, rung-2 adoption, `ConfirmRunning`
of a live found pane, and the pane being cleared at the start of every restart. Unverified premise: that the agent
window can be absent while a tab survives through anything other than direct use of the private tmux server. 078 found
no Farhelm path that produces it; 105 relies on the code's own statements that the shape is real.

Mechanism:

- `crates/farhelm-supervisor/src/tmux.rs:4140-4188`: `pane_states` skips the window-marker query whenever no tmux
  session on the server has more than one window, on the stated grounds that "a session with one window can have no
  tab". Without that query every pane's `tab` and `agent` markers are `None`
  (`crates/farhelm-supervisor/src/tmux/control_codec.rs:160-172`). `pane_states_with_markers` (tmux.rs:4190-4195) says
  the opposite: "A dead-agent restart is the deliberate exception: after the recorded pane disappears, a surviving tab
  can be the session's only window, and its marker is needed to distinguish it from an unowned window." Restart and
  delete read markers (`crates/farhelm-supervisor/src/service/core.rs:12773-12782`, 12883); the two consumers below do
  not.
- Reload: `reload_sessions` reads `tmux.pane_states()` (core.rs:6028) and, for a row with an empty recorded pane, calls
  `agent_pane_from_states` (core.rs:6061-6066). Its rung 2
  (`crates/farhelm-supervisor/src/service/terminals.rs:348-352`) picks the lowest pane with
  `agent.is_none() && tab.is_none()`, which matches the surviving tab when markers were skipped. Its own docs
  (terminals.rs:314-316) say adopting a tab's window means "stop would reap the tab, restart would respawn into it". A
  live found pane on a row that is not confirmed Running reaches `Transition::ConfirmRunning { pane }`
  (core.rs:6315-6330), durably recording the tab's pane as the agent pane. The staleness rule at core.rs:6292-6293
  covers only a dead pane, and the comment at core.rs:6320-6323 assumes a live pane found after a crashed relaunch must
  be the respawn's.
- Report anchoring: `owned_pane_anchor` (core.rs:15675-15696) does the same during any publication gap. The anchor
  validates conversation reports, so in this state a report whose process chain reaches the tab's shell anchors to the
  wrong pane.
- Startup spare-claim protection (core.rs:6922-6924) uses the same path, but only to protect claims, which errs in the
  safe direction.

How a row ends up pane-less: `begin_relaunch` opens the new generation with `pane = ''` and `Launching` before any tmux
side effect (`crates/farhelm-supervisor/src/store.rs:4141-4170`). For an agent pane that is gone while tabs live,
Restart keeps the existing tmux session and adds a window (core.rs:11870-11887), after detaching and reading tabs,
credentials and `has_session` (core.rs:11570-11580, 11805-11912). The row stays pane-less across a supervisor stop
(update, crash, OOM, host-service restart) between the generation commit and the spawn, after an ambiguous relaunch
failure (core.rs:11651-11653, 11662-11670; store.rs:4185-4195, "stay `Launching` for reload to reconcile"), or when
`abort_relaunch` fails (core.rs:11700-11705). The "no session has more than one window" precondition is the common case
here: the affected tmux session now holds only the tab, and sessions without tabs have one window each.

Reachability of the agent-window-absent shape, per 078: global `remain-on-exit on` keeps dead agent panes
(tmux.rs:2319); agents and tabs run without `TMUX`/`TMUX_PANE` (`crates/farhelm-supervisor/src/launch.rs:130-143`); the
only Farhelm kill of an agent window happens after the replacement is confirmed (core.rs:12177-12182); unwind kills only
the replacement window (core.rs:12295-12305, 12448). The independent reviewer found the same and noted that the code's
docs still assert the shape is reachable (tmux.rs:4192-4195, core.rs:12773-12776), and that tmux.rs:4147-4150
contradicts tmux.rs:4192-4195 either way.

Consequence: Stop reaps the recorded pane's process tree, which is now the user's tab. Restart respawns into it, killing
the tab's shell and its children. The Delete guard that counts open tabs (core.rs:12783-12791) excludes the recorded
pane, so an unconfirmed Delete no longer warns about that tab. Prompts meant for the agent go to the tab's shell.

How to verify: at unit level, give `agent_pane_from_states` a map shaped as `pane_states` returns it with markers
skipped (one session, one live window whose tab marker was not read) and an empty recorded pane; it returns the tab
pane. End to end: create a session, open a tab, remove the agent window through the private socket, call
`store.begin_relaunch(...)` (or stop the supervisor after it commits and before `new-window`), make sure no other tmux
session has two windows, then start a new supervisor. The session reads Running with the tab's pane as its agent
terminal.

Fix sketch: read markers unconditionally wherever pane-less recovery runs (`reload_sessions` and `owned_pane_anchor`),
at least whenever some row has an empty pane, as the delete guard and `old_agent_window_anchor` already do. Correct the
tmux.rs:4147-4150 rationale. Optionally refuse rung-2 adoption on generation > 0 rows when the session has tab-marked
panes. The cost is one more tmux round trip, only when some row is pane-less.

Possible cover, for triage to judge:

- SPEC.md "Maintainer-confirmed decisions" > "Ownership during cleanup and provisioning": "Windows, panes, processes and
  configuration changes made through the private server anyway are outside every Farhelm guarantee, including cleanup on
  Stop, Restart, Delete and tab close. Do not add code or complexity to detect, track, clean up after, or recover from
  them." This covers the item only if removing the agent window needs such out-of-band use.
- TRIAGE_OUTCOMES.md "pane-states-skips-markers-single-window.md" (outcome `other`, skipped as covered by that section,
  "not re-verified for correctness") closed an earlier finding about the same marker skip on the premise that "The
  trigger is a user or a program in a session using the private tmux server directly". Recording found this ledger entry
  after the independent reviewer's verdict. This item adds the ordinary-use routes to a pane-less row (supervisor
  restart mid-Restart, ambiguous restart failure) and the report-anchoring consumer, but the agent-window-absent premise
  is the same one that decision rested on.
- SPEC.md "Local authority and trust between hosts": same-account interference with the private session is trusted, and
  only accidental interference is guarded.
- FILTER.md "Session status and history after a crash or a partly failed operation" matches the trigger only; its
  exclusions (a destructive action applied to the wrong target, loss of processes) keep it from matching.
