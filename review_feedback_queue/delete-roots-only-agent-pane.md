# Delete roots its process walk only on the agent pane

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Deleting a session can report success while programs started in its tabs (on a Mac: a backgrounded `ssh -N`,
`caffeinate`, a nohup'd command), in split panes, or in a not-yet-reattached session keep running, unreachable
afterwards.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F12 / COR-DELETE-ROOTS`, tagged **possible**. Anchors and title: `service/teardown.rs:224-236`,
`service/teardown.rs:294-303`, `service/sweep.rs:1264-1271`, `service/sweep.rs:294-304` — Delete roots its process walk
only on the agent pane, so tab-pane, split-pane and terminal-less-session processes without readable markers survive
Delete

The kill sweep finds a session's processes two ways. It walks down the parent-pid tree from one root process, the "PPID
closure", and it scans every same-user process's environment for the `FARHELM_SESSION_ID` marker Farhelm sets at launch.
Close Tab roots the walk at the tab's own pane. Delete (`teardown_session`) passes only the agent pane's pid as root
(teardown.rs:224-236, teardown.rs:294-303; `reap_process_tree` accepts a single `Option<u32>` root, sweep.rs:1264-1271).
The in-code justification is that "a tab's shell carries the session marker … and the marker scan finds it wherever it
is".

That justification fails in four cases:

- **(a) macOS 26+.** The project's own note (sweep.rs:294-304, and SPEC_impl) records that macOS withholds the
  environment of Apple platform binaries even from the same user. A tab runs `env -u … $SHELL -l -i`, so `/usr/bin/env`
  and then `/bin/zsh` are platform binaries, and so are common children like `ssh`, `caffeinate` and `tail`. They read
  as marker-less. The documented mitigation is that the PPID closure still reaches them while they stay in the pane's
  tree, but Delete never roots a closure on a tab pane.
- **(b) Linux without a user manager.** A tab descendant that scrubbed its environment but is still under the tab shell
  is reachable only by a walk from the tab pane, which Delete does not do.
- **(c) Hand-split panes,** in a tab or in the agent window. The tmux server spawns them without the markers Farhelm
  passed to the original window with `-e` (reviewer-verified), outside any scope and not under the agent pane.
- **(d) A session with no recorded terminal.** It has no root at all (`root_pid = None`), so even the agent pane's own
  tree is swept by marker only.

After the sweep reports "confirmed gone", tmux `kill-session` only sends SIGHUP to each pane's foreground process group.
Jobs started with `nohup`, disowned, or run under `setsid` survive, and the row is deleted.

SPEC.md says Delete terminates "the agent and tabs if running" and that teardown covers "ordinary agent descendants,
including background servers". Here Delete covers less than a single Close Tab does, reports success, and leaves nothing
to retry from.

The premise for (c) is whether hand-split panes count as the session's terminals; SPEC acknowledges hand-split tabs.
Case (a) is definite on macOS 26+ per the project's own note. The fix: before sweeping, list every live pane in the
session's tmux session and pass each owned pane pid as an additional closure root, generalizing
`reap_process_tree`/`kill_process_tree` from `Option<u32>` to a list of validated roots. The pane list is already
fetched through `pane_states`/`session_tabs_including_dead`; for terminal-less rows, use the durable name.
Alternatively, put the session marker into the tmux session environment so later panes inherit it. At minimum, correct
the comments in teardown.rs and sweep.rs.

For the user, deleting a session can report success while programs started in its tabs, split panes, or a
not-yet-reattached session keep running with no way left to reach them. On a Mac that includes a backgrounded `ssh -N`,
`caffeinate`, or a nohup'd command.
