# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing f6519d] `launcher-folder-shortcuts.md` — replace the launcher's folder links with a dropdown on the folder field
- [in-flight 4a94b2] `flickering-path-hover.md` — keep a path hover steady while the terminal redraws under it
