# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `gui-saves-last-write-wins.md` — Accept last-write-wins for GUI template and number-key saves in SPEC.md, and stop a failed number-key save from later writing its stale map over newer agent changes.
- [in-flight 049983] `bind-gui-answers.md` — Bind every GUI answer to what the user saw: one shared prompt mechanism with opening tokens, click-time preconditions, declarative link covers and retired held input, plus the six GUI outcomes folded in at the 2026-10-21 triage (after `popover-anchor-close.md`)
