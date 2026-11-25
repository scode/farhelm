# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `drift-permission-modes.md` — Add live permission mode and custom permission flag drift tests across supporting agents (after `drift-model-selection.md`)
- [complete] `drift-initial-prompt-launch.md` — Add live launch-time initial prompt delivery drift tests across supporting agents (after `drift-model-selection.md`)
- [complete] `drift-named-conversation-starts.md` — Add live one-time named conversation start drift tests across all built-in agents (after `drift-model-selection.md`)
- [in-flight ed6024] `drift-capture-timing-hooks.md` — Add live conversation capture timing, hook payload, and hooks-disabled drift tests (after `drift-model-selection.md`)
- [in-flight a5d53a] `drift-workspace-trust.md` — Add live workspace trust pre-trust drift tests across supporting agents (after `drift-model-selection.md`)
- [in-flight 99b304] `drift-instructions-delivery.md` — Add live `$farhelm` instructions pointer delivery drift tests across supporting agents (after `drift-model-selection.md`)
