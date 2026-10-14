# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight a668d1] `release-advisories.md` — A signed advisories file on get.farhelm.io that the desktop updater reads, starting with release revocation
- [landing f3c9a7] `custom-permission-flags.md` — A "custom" permission choice that passes the user's own permission flags and always counts as YOLO (after `guarded-permission-mode.md`)
- [pending] `launch-with-conversation-id.md` — Let New, Clone, Replace with and Restart with start in a conversation the user names
