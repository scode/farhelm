# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `launcher-command-field.md` — make the session launcher's search field its header and starting point, with hints, a left column of recent setups, templates and results, a calmer form, and no footer summary
- [complete] `golden-changes-page.md` — write a page of only the changed and new golden pictures (reference, new, differing pixels) after every run that finds any, and describe one visual-review procedure built on it (after `goldens-fast-and-targeted.md`)
