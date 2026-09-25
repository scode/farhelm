# Closing a tab kills its window before detaching the viewer

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In a small timing window, closing a tab shows "terminal input failed" instead of "terminal tab closed".

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as
`F13 / COR-CLOSE-TAB-ORDER`, tagged **definite**. Anchors and title: `service/core.rs:11736-11759`,
`service/connection.rs:503-531` — Closing a tab kills its window before detaching its viewer

`close_tab_window` closes a tab in four steps (core.rs:11736-11759). It reaps the tab's processes, kills the tab's tmux
window, runs a second, marker-only process sweep that can take up to the SIGTERM grace period, and only then calls
`detach_closed_tab`. That last step removes the viewer's attachment and sends it the "terminal tab closed" notice. The
tab's input client stays live until then. A keystroke that arrives after the window kill, or a terminal emulator's
automatic reply to a query, is sent as `send-keys` to a pane that no longer exists. tmux answers "can't find pane".
Reviewers verified that the input does not fall back to the agent's pane, which refutes the worse suspicion in the scope
notes.

The input path in the connection handler (connection.rs:503-531) treats that failure as a broken attachment. It logs a
warn-level "input dropped", removes the attachment, waits for the forwarder task to finish while still holding the
supervisor-wide `attachments` lock, and tells the viewer `Detached("terminal input failed: … can't find pane: %N")`.
`detach_closed_tab` then finds nothing to detach and returns Ok. A normal close in that window therefore produces a
failure-shaped reason instead of "terminal tab closed", a misleading warning in the log, and a forwarder join under the
lock that every session's input shares. The suggested change is to remove the tab's attachment, or at least close its
input route, before `kill_window`, while keeping the unconditional detach inside the same supervisor-owned task. An
alternative is to have the input path recognize a tab that is closing and drop the frame silently.
