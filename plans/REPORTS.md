# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`alias-in-add-host`](reports/alias-in-add-host.report.md) landed 2026-10-23 in #2666, #2667: The add host dialog takes an optional alias, written with the host as it is added; adding a destination that is already a host is refused, naming that host
