# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 95d641] `pin-sessions.md` — Pin sessions to the top of the sidebar from the row menu: pin icon, separator, shared across clients, carried over on Replace
- [in-flight 89bf7b] `detect-mid-session-yolo.md` — The shield turns YOLO when an agent's status line shows its no-checks mode (Claude, Grok, Muse, Cursor), best effort
- [in-flight fc0112] `ordered-gui-helm-requests.md` — The GUI holds a request back only until its own earlier configuration saves are answered, fixing two save/read races
- [pending] `claude-trust-prompt.md` — Opt-in setting to pre-trust managed checkouts in Claude's config, with a hint band when Claude asks
