# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `tmux-drop-from-docs.md` — Specs, docs, comments and rationales describe one kind of terminal, the remaining TODO.md wording is updated, and a final audit accounts for every remaining tmux and xterm.js mention (after `tmux-drop-from-tooling.md`)
