# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing e95fc1] `browser-oracle-and-fixture-fixes.md` — 12 triage outcomes: browser specs that can pass while their behavior is broken, or skip cleanup after a failure (one test-only PR)
- [landing 107978] `dev-tooling-robustness-fixes.md` — 9 triage outcomes: small robustness fixes in deflake, the recorder, the release smoke check, the brick test, the preview script, README hero staging and a tmux test helper; each discarded instead if its fix is not small
- [landing 107978] `triage-bookkeeping-round-two.md` — 11 triage outcomes: remove 9 discarded review-queue findings, fold the create-rollback case into the systemd TODO entry, and add a Maybe later TODO for macOS mixed-scale window restore (one PR, no code or spec change)
- [pending] `ui-retry-and-notice-fixes.md` — 3 triage outcomes: Replace retries reuse their request key, an "outcome unknown" uninstall notice when the window never saw it finish, and a note when a paste leaves an item out (after `ui-layout-and-escaping-fixes.md`)
