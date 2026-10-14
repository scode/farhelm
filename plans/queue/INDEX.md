# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight b07e14] `update-check-readout.md` — Click the desktop app's version readout to check for updates (with a ↻ hint), and show automatic update failures that persist
- [pending] `release-advisories.md` — A signed advisories file on get.farhelm.io that the desktop updater reads, starting with release revocation
- [in-flight afc614] `guarded-permission-mode.md` — Rename non-YOLO to "guarded", force each agent's guarded mode with explicit flags, and explain it per agent
- [in-flight 0bdf2b] `confirmation-focus-and-skip.md` — Enter goes ahead on every confirmation (except YOLO), and a Settings switch skips the everyday session confirmations
- [pending] `quick-launch-template.md` — Mark one template ⚡ quick launch; Cmd+P / Ctrl+Shift+P starts a session from it with no launcher
- [pending] `custom-permission-flags.md` — A "custom" permission choice that passes the user's own permission flags and always counts as YOLO (after `guarded-permission-mode.md`)
- [pending] `launch-with-conversation-id.md` — Let New, Clone, Replace with and Restart with start in a conversation the user names
