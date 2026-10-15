# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`compact-rows-test`](reports/compact-rows-test.report.md) landed 2026-10-15 in #2031: update the compact-rows and OMP composer browser tests to the permission wording since the guarded mode
- [`settings-lockout-scope`](reports/settings-lockout-scope.report.md) landed 2026-10-15 in #2032: keep other hosts' Settings usable during a one-click Update; block it only while a setup or uninstall confirmation may open
