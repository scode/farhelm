# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `triage-trust-settings-and-ui-wording.md` — Execute 7 triage outcomes for best-effort Custom auto-trust flags, unread Settings switch a11y, persistent-host trust help, disposable trust hints, Restart card trust copy, and trust test gaps (after `triage-settings-nov24.md`)
- [pending] `triage-launcher-safety.md` — Execute 5 triage outcomes for Replace-with Recent screen-reader wording, New/Clone vanished-pick Enter guards, held Escape, stale permanent YOLO feedback, and repository-history load errors (after `launcher-consent-and-custom-agent-fixes.md`)
