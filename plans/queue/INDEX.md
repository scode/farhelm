# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 51db96] `bulk-session-actions.md` — Shift+click and Cmd/Ctrl+click select several sessions; a bar offers Stop, Delete, Pin/Unpin and Mark read/unread, with one structured confirmation for a bulk Delete
- [pending] `host-filter-feedback.md` — A host chosen in the rail shows as a removable chip beside the status filter, and an empty filtered list names its host and status
