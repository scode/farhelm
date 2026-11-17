# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 10e439] `gpui-last-mile-fixes.md` — GPUI last mile: suspended-host launch refusal wording, suspended card provider words, Clone copied-text escape, and the launcher's remaining parity tests
- [pending] `gpui-shared-placement.md` — GPUI last mile: one shared placement picker for the launcher and the template editor (after `gpui-last-mile-fixes.md`)
- [in-flight 333aa6] `gpui-docs-shots-mac.md` — GPUI last mile: capture the docs screenshots on the test Mac, and prove the hero and docs captures there
- [pending] `gpui-docs-followups.md` — GPUI last mile: docs for environment recent setups and the suspended-host launch refusal (after `gpui-last-mile-fixes.md`)
