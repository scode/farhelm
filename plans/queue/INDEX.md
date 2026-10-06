# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.
- [complete] `home-tab-trailing-slash.md` — A terminal tab beside a session in ~ starts in ~, not ~/ (strip the trailing slash handed to tmux).
- [in-flight c57da1] `new-session-shortcut.md` — Cmd+N opens a new session in the Mac desktop app.
- [in-flight fb69b2] `installer-feedback-prompt.md` — A fresh install's closing message invites feedback, with the agreed wording.
