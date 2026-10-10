# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`e2e-helm-test-oracles`](reports/e2e-helm-test-oracles.report.md) landed 2026-10-10 in #1818: Stop-sweep tests run without cgroup scopes, and installer, provisioning, upload, relay, list-race, stale-refresh and malformed-message tests assert what they claim
- [`browser-sidebar-test-oracles`](reports/browser-sidebar-test-oracles.report.md) landed 2026-10-10 in #1821: sidebar, notification, create-retry, multihost and harness browser tests assert what they claim, and failed tests stop leaking sessions, hosts, supervisors and held routes
