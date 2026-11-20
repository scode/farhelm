# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `shell-sessions.md` — Bash and Zsh as session types beside the agents, with no permission mode (protocol bump)
- [landing 401629] `desktop-login-shell-path.md` — The desktop app runs ssh and everything else with the login shell's PATH, even from the Dock; a bell notice when it cannot be read
