# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`launcher-gpt-6-2-models`](reports/launcher-gpt-6-2-models.report.md) landed 2026-11-18 in #4099: The launcher suggests gpt-6.2-sol and gpt-6.2-astra in place of gpt-6.1-sol and gpt-6-astra for every harness that offers them, and astra gains the full Codex effort list
- [`mute-sessions`](reports/mute-sessions.report.md) landed 2026-11-18 in #4104, #4106, #4107, #4108: Mute a session for an hour, until 06:00 or until unmuted, so it stays grey and new work does not promote it, while waiting still breaks through
- [`session-groups`](reports/session-groups.report.md) landed 2026-11-18 in #4110, #4111, #4112, #4113, #4114: Named, collapsible session groups in a manual order below the global pins, each with its own pins and, when collapsed, its most urgent member's dot
