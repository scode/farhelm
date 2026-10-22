# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `bind-gui-answers.md` — Bind every GUI answer to what the user saw: one shared prompt mechanism with opening tokens, click-time preconditions, declarative link covers and retired held input, plus the six GUI outcomes folded in at the 2026-10-21 triage (after `popover-anchor-close.md`)
- [in-flight eb23d7] `test-hygiene-oct21.md` — Eight test-only fixes from the 2026-10-21 auto-triage: cleanup armed as soon as a fixture exists, tests that prove the acceptance they claim, and a deadline that bounds each approval poll.
- [pending] `duplicate-uninstall-retry-advice.md` — Once the other entry reaching the same machine is removed, the duplicate's uninstall refusal says to press Retry and uninstall through this entry, instead of naming the removed entry.
