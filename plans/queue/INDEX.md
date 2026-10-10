# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.
- [landing 5df5f5] `approval-card-layout.md` — the agent request card moves to the top of the main pane, compact, one request expanded at a time
- [landing 5df5f5] `resizable-sidebar.md` — drag the sidebar's edge to resize it (240–600px), remembered per device
- [complete] `sweep-on-timer.md` — the supervisor sweeps on its 2 s timer only, and stops re-reading settled stopped sessions
- [in-flight 716d07] `host-icons.md` — remote hosts get a chosen icon and color, shown everywhere a host is named; the host settings dialog is refreshed and the launcher gets a custom host picker
- [in-flight 79e3d3] `waiting-sound.md` — sounds when a session waits, an approval arrives, or (opt-in) a turn finishes, each switchable per device in Settings
- [in-flight 768904] `terminal-file-download.md` — path-shaped text in a terminal is checked on hover and downloads from the session's host on click (100 MB limit; desktop saves to Downloads)
- [pending] `hook-report-watch.md` — the supervisor applies conversation reports as soon as they are written, via a file watch, with the timer as backstop (after `sweep-on-timer.md`)
