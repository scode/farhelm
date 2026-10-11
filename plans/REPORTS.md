# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`checkout-folder-and-ui-fixes`](reports/checkout-folder-and-ui-fixes.report.md) landed 2026-10-10 in #1824, #1825, #1826, #1827, #1828, #1830, #1831: the managed-checkout folder becomes a Settings setting that the launcher can also set on the spot and that is created on first checkout, plus approval-card, save-as-template and Settings-switch layout fixes
- [`test-portability-fixes`](reports/test-portability-fixes.report.md) landed 2026-10-10 in #1834: tests stop passing vacuously on macOS (/proc liveness, stat flags) or large-page Linux, stop failing on symlinked temp paths, and GNU-only provisioning tests are gated to Linux
