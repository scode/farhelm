# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 568344] `blocked-notify-attention-and-debounce.md` — Move blocked-agent alerts to the red waiting indicator and sound with an ntfy awareness grace period
- [pending] `ntfy-quiet-hours-urgency.md` — Add local-time quiet hours and quiet-hours priority/off setting for ntfy alerts
