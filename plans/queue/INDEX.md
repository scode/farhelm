# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight a668d1] `release-advisories.md` — A signed advisories file on get.farhelm.io that the desktop updater reads, starting with release revocation
- [in-flight d07143] `launch-with-conversation-id.md` — Let New, Clone, Replace with and Restart with start in a conversation the user names
- [complete] `release-notes-menu-item.md` — A "release notes" item in the ? menu that opens the release notes on the docs website
- [in-flight 8859e1] `template-number-shortcuts.md` — Bind templates to number keys; Cmd+digit in the Mac app, Ctrl+Shift+digit elsewhere, launches directly like quick launch
- [pending] `intro-tour.md` — A welcome card on first open per helm, pointing at Get started and Send feedback; reopened from the ? menu
