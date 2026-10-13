# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 9af1b7] `codex-resident-launcher.md` — capture a Codex session's conversation through a launcher named `codex` that stays running as Codex's parent, by pinning the first reporting Codex process
- [pending] `feedback-dialog-polish.md` — restyle the send feedback dialog to match the approved mockup: header with ×, dark fields, quiet labels, pinned footer
- [pending] `open-links-in-browser.md` — let agents on remote hosts open links in the user's browser through BROWSER, an approval card, and two per-host "open links without asking" settings
