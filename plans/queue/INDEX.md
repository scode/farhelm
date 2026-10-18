# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `host-terminal-colors.md` — local host picks its icon and color, red for every host, optional host-colored terminal frame
- [in-flight acd048] `agent-wrappers.md` — optional command prefix for agent launches, treated exactly like the real agent
- [pending] `agent-quick-launch-templates.md` — agents can set the quick-launch template and number keys through the farhelm command
