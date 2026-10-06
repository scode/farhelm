# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`mac-release-test`](reports/mac-release-test.report.md) landed 2026-10-06 in #1682, #1683: Agent-driven Mac release test, first slice: the updater's test-only "latest" override, the per-release Tart VM recipe, and the bring-up document for the agent on the Mac host.
