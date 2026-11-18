# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`session-groups`](reports/session-groups.report.md) landed 2026-11-18 in #4110, #4111, #4112, #4113, #4114: Named, collapsible session groups in a manual order below the global pins, each with its own pins and, when collapsed, its most urgent member's dot
