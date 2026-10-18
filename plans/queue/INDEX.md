# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 72c227] `repeat-quick-launch.md` — templates name sessions with a base name (numbered when taken) or a fixed name, so a named template can be quick-launched repeatedly into managed checkouts
- [pending] `host-uninstall-dialogs.md` — host setup, update and uninstall show their steps as a stepper in the dialog and in the row's progress, refusals open in dialogs, and the post-uninstall notice shrinks to one line
