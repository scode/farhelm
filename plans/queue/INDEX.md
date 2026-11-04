# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `in-app-announcements.md` — An announcement queue the app shows once per helm, fed for now by announcements built into releases
- [landing 2f8d9d] `session-summary-beside-list.md` — Show the session hover summary card beside the session list, level with the row, instead of over neighbouring rows
