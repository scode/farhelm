# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `triage-launcher-safety.md` — Execute 5 triage outcomes for Replace-with Recent screen-reader wording, New/Clone vanished-pick Enter guards, held Escape, stale permanent YOLO feedback, and repository-history load errors (after `launcher-consent-and-custom-agent-fixes.md`)
