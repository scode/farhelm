# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`triage-tooling-output-ownership`](reports/triage-tooling-output-ownership.report.md) landed 2026-10-05 in #1649, #1650, #1651: Maintainer video recorder and screenshot publisher delete or drop only what they made.
- [`triage-install-uninstall-gaps`](reports/triage-install-uninstall-gaps.report.md) landed 2026-10-05 in #1654, #1655, #1656: Escape remote linger errors, exact uninstall identity check, installer refuses symlinked app folders.
- [`triage-template-gaps`](reports/triage-template-gaps.report.md) landed 2026-10-05 in #1652, #1653: Templates refuse to overwrite on an unloaded list and refuse a replaced machine at dispatch.
