# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 268823] `supervisor-lifecycle-test-fixes.md` — make seven supervisor lifecycle tests clean up their fixtures and prove the paths they name
- [complete] `helm-and-portability-test-fixes.md` — make five helm routing, upload, portability and timing tests prove what they claim
- [landing 268823] `e2e-session-test-fixes.md` — make seven end-to-end terminal and session tests prove what they claim
- [in-flight 39e34e] `browser-spec-fixes.md` — recover credentials after a failed rotation test, tighten the long-folder sidebar test, and clean up after the real-Claude spawn test
