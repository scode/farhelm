# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.
- [landing de4c41] `test-portability-fixes.md` — tests stop passing vacuously on macOS (/proc liveness, stat flags) or large-page Linux, stop failing on symlinked temp paths, and GNU-only provisioning tests are gated to Linux
