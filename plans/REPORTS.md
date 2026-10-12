# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`supervisor-lifecycle-test-fixes`](reports/supervisor-lifecycle-test-fixes.report.md) landed 2026-10-11 in #1910: make seven supervisor lifecycle tests clean up their fixtures and prove the paths they name
- [`e2e-session-test-fixes`](reports/e2e-session-test-fixes.report.md) landed 2026-10-11 in #1911: make seven end-to-end terminal and session tests prove what they claim
- [`helm-and-portability-test-fixes`](reports/helm-and-portability-test-fixes.report.md) landed 2026-10-11 in #1912: make five helm routing, upload, portability and timing tests prove what they claim
- [`browser-spec-fixes`](reports/browser-spec-fixes.report.md) landed 2026-10-11 in #1913: recover credentials after a failed rotation test, tighten the long-folder sidebar test, and clean up after the real-Claude spawn test
