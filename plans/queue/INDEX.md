# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 184b35] `settings-wait-while-saving.md` — Make Settings Close/Escape and editors wait for saves, rip out slow-helm concurrency, and finish surviving Settings follow-ups (after `custom-agent-delete-prompt.md`)
- [landing c5fb4b] `terminal-tab-hover-close.md` — Move shell tab close button to a fixed left icon slot revealed on hover
- [pending] `agent-blocked-notifications.md` — Fix ntfy waiting alerts and add blocked-agent notifications with instruction injection (after `settings-wait-while-saving.md`)
