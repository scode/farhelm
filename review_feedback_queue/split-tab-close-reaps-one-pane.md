# Closing a hand-split tab reaps only one pane

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Closing a split tab can leave a server started in its other half running, with no error.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F13 / COR-SPLIT-TAB`, tagged **possible**. Anchors and title: `service/core.rs:12006-12014`,
`service/terminals.rs:257-273`, `service/sweep.rs:251-260` — Closing a hand-split tab reaps only one pane's process
tree; background jobs from the other panes survive

A terminal tab is a tmux window. A user can split that window into several panes through the session's own tmux access,
and SPEC treats the result as one tab. `tabs_from_pane_states` (terminals.rs:257-273) does too: it identifies a tab by a
window-level tmux option, groups all of the window's panes under it, and picks one "handle" pane, the lowest live one.

`reap_tab_tree`, the kill routine behind Close Tab and the ticker's automatic reap of exited tabs, roots its parent-pid
walk at that single handle pane (core.rs:12006-12014). It then selects everything else by the tab's environment marker
(`SweepTarget::Tab` requires the session marker plus this tab's id, sweep.rs:251-260). A pane added with `split-window`
misses all three mechanisms:

- It does not inherit the markers Farhelm passed to the tab's window with `new-window -e` (reviewer-verified).
- It is a child of the tmux server, not of the handle pane's shell.
- It sits outside the tab's systemd scope.

So neither the walk, the marker scan, nor the scope kill reaches it. The subsequent `kill-window` only sends SIGHUP to
each pane's foreground job. Jobs started with `nohup`, `setsid` or `disown`, and processes that ignore SIGHUP, survive.
The close still reports that nothing of the tab is left running, and the ticker's automatic reap inherits the same gap.

SPEC says closing a tab "kills that shell and its processes". A supported tab shape silently falls outside that promise.

The open premise is whether SPEC's promise covers a hand-split tab's other panes. The fix: in `reap_tab_tree` (the
`PaneIfLive` path), list every pane of the tab's window from the same `pane_states` data and use each live, owned pane
pid as a closure root. Alternatively, set the tab markers as window-scoped tmux environment so split panes inherit them.

For the user, closing a split tab can leave a server started in its other half running, with no error.
