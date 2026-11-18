# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 12d940] `launcher-gpt-6-2-models.md` — The launcher suggests gpt-6.2-sol and gpt-6.2-astra in place of gpt-6.1-sol and gpt-6-astra for every harness that offers them, and astra gains the full Codex effort list
- [in-flight 3a0802] `mute-sessions.md` — Mute a session for an hour, until 06:00 or until unmuted, so it stays grey and new work does not promote it, while waiting still breaks through
- [pending] `session-groups.md` — Named, collapsible session groups in a manual order below the global pins, each with its own pins and, when collapsed, its most urgent member's dot (after `mute-sessions.md`)
