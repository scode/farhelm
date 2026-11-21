# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 0d4a6c] `agent-hook-setup.md` — give every agent type its own Settings section, and let Grok and Antigravity users opt in once to Farhelm installing its Restart hooks into those agents on every machine, asked in the launcher and explained on the greyed Restart (after `launcher-command-field.md`)
