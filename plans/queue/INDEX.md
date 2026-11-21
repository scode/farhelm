# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `launcher-repository-search-loading.md` — show "Loading repositories…" instead of "no matches" while the launcher's repository search is pending, and close its triage outcome
- [complete] `stop-wait-starting-launch-witness.md` — make the supervisor stop-wait test witness a launch still registering, and close its triage outcome
