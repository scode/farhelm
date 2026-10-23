# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 2d8582] `restart-with-as-launcher.md` — Restart with becomes a mode of the session launcher, with agent type, host and folder held fixed and templates offered as in Replace with; the separate dialog goes away (after `session-view-fixes-oct22.md`, `resume-target-fixes-oct22.md`, `launch-refusal-visibility.md`, `per-host-last-folder.md`)
