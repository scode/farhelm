# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [pending] `agent-delete-and-prompt.md` — `farhelm agent delete` (approved, no self-delete), and an initial prompt at creation from the CLI, the launcher and templates
- [pending] `template-name-escape.md` — Template editor: an older template's unsafe session name shows escaped and read-only, like model, folder and commands
- [pending] `webkit-ime-guards.md` — Input methods in the Mac app and Safari: the key that ends a composition no longer submits or closes, across every guard
- [pending] `calm-session-unavailable.md` — Session pane: one calm "session unavailable" card instead of red and amber error text whenever the supervisor is out of reach
