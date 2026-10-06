# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.
- [complete] `new-session-shortcut.md` — Cmd+N opens a new session in the Mac desktop app.
- [complete] `installer-feedback-prompt.md` — A fresh install's closing message invites feedback, with the agreed wording.
