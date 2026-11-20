# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 29e719] `shell-sessions.md` — Bash and Zsh as session types beside the agents, with no permission mode (protocol bump)
- [complete] `tensorlake-pause-attribution.md` — A later Tensorlake pause no longer shows Farhelm's old suspend label and time
