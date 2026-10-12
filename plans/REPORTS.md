# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`review-queue-discards`](reports/review-queue-discards.report.md) landed 2026-10-12 in #1921: Remove the 21 discarded review-queue findings and record their execution (one PR, no code or spec change)
- [`host-trust-and-tooling-spec`](reports/host-trust-and-tooling-spec.report.md) landed 2026-10-12 in #1923: Spec clarifications for 12 triage outcomes: host denial of service accepted, one run per checkout, dedicated ownership of managed checkouts, three setup/uninstall acceptances
- [`e2e-oracle-and-cleanup-fixes`](reports/e2e-oracle-and-cleanup-fixes.report.md) landed 2026-10-12 in #1925: 13 triage outcomes: end-to-end test oracles that can pass or fail wrongly, fixture cleanup on failure paths, and gating the e2e suite to Linux
