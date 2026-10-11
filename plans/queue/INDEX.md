# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 8e5eb5] `helm-and-portability-test-fixes.md` — make five helm routing, upload, portability and timing tests prove what they claim
- [landing 8e5eb5] `browser-spec-fixes.md` — recover credentials after a failed rotation test, tighten the long-folder sidebar test, and clean up after the real-Claude spawn test
