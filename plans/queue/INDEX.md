# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `template-number-shortcuts.md` — Bind templates to number keys; Cmd+digit in the Mac app, Ctrl+Shift+digit elsewhere, launches directly like quick launch
- [pending] `intro-tour.md` — A welcome card on first open per helm, pointing at Get started and Send feedback; reopened from the ? menu
- [pending] `ssh-2fa-help.md` — A ? menu dialog (and add-host link) showing the ssh commands that start a 2FA host's two shared connections by hand
