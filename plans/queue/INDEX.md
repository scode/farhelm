# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing e95fc1] `browser-oracle-and-fixture-fixes.md` — 12 triage outcomes: browser specs that can pass while their behavior is broken, or skip cleanup after a failure (one test-only PR)
- [in-flight 1eab40] `ui-retry-and-notice-fixes.md` — 3 triage outcomes: Replace retries reuse their request key, an "outcome unknown" uninstall notice when the window never saw it finish, and a note when a paste leaves an item out (after `ui-layout-and-escaping-fixes.md`)
