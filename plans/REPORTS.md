# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`helm-and-portability-test-fixes`](reports/helm-and-portability-test-fixes.report.md) landed 2026-10-11 in #1912: make five helm routing, upload, portability and timing tests prove what they claim
- [`browser-spec-fixes`](reports/browser-spec-fixes.report.md) landed 2026-10-11 in #1913: recover credentials after a failed rotation test, tighten the long-folder sidebar test, and clean up after the real-Claude spawn test
