# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`browser-sidebar-test-oracles`](reports/browser-sidebar-test-oracles.report.md) landed 2026-10-10 in #1821: sidebar, notification, create-retry, multihost and harness browser tests assert what they claim, and failed tests stop leaking sessions, hosts, supervisors and held routes
