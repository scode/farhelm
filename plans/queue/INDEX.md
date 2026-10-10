# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.
- [in-flight 274382] `approval-card-layout.md` — the agent request card moves to the top of the main pane, compact, one request expanded at a time
- [in-flight a9446b] `resizable-sidebar.md` — drag the sidebar's edge to resize it (240–600px), remembered per device
- [pending] `sweep-on-timer.md` — the supervisor sweeps on its 2 s timer only, and stops re-reading settled stopped sessions
