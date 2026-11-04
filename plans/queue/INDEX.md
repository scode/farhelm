# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `vs16-emoji-width.md` — Grid terminals: size emoji variation sequences (U+FE0F, U+FE0E) the way Ghostty does
- [landing 338809] `grid-drop-and-drag-notice.md` — Grid terminals: accept dropped files and show the drag-copy notice, placed 3 rows off the pointer
- [pending] `in-app-announcements.md` — An announcement queue the app shows once per helm, fed for now by announcements built into releases
- [complete] `launch-progress.md` — Show a live "Launching on host… 14 s" status while a launch, Restart or Replace waits
- [complete] `session-summary-beside-list.md` — Show the session hover summary card beside the session list, level with the row, instead of over neighbouring rows
