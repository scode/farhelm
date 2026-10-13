# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing be9ce6] `codex-resident-launcher.md` — capture a Codex session's conversation through a launcher named `codex` that stays running as Codex's parent, by pinning the first reporting Codex process
- [in-flight d7bc5d] `feedback-dialog-polish.md` — restyle the send feedback dialog to match the approved mockup: header with ×, dark fields, quiet labels, pinned footer
- [in-flight 6bfd94] `open-links-in-browser.md` — let agents on remote hosts open links in the user's browser through BROWSER, an approval card, and two per-host "open links without asking" settings
- [pending] `app-settings-pinned.md` — pin the app settings dialog's header and footer, add an × close, and group the settings into sections, per the approved mockup (after `feedback-dialog-polish.md`)
