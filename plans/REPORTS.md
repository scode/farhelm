# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`delete-one-trashed-checkout`](reports/delete-one-trashed-checkout.report.md) landed 2026-10-11 in #1888: a delete button on each checkout in the trash dialog, deleting it at once without a confirmation
- [`update-check-feedback`](reports/update-check-feedback.report.md) landed 2026-10-11 in #1889: show a user-started update check's progress and outcome inline on the desktop app's version readout
- [`installer-and-font-license-fixes`](reports/installer-and-font-license-fixes.report.md) landed 2026-10-11 in #1906: fix three installer and docs-site bugs: a directory at a file destination, an incomplete latest-version answer, and the font license the UI omits
- [`bell-build-mismatch-fixes`](reports/bell-build-mismatch-fixes.report.md) landed 2026-10-11 in #1907: keep the notification bell and the read/unread toggle current while the helm and the page are different builds
- [`launcher-and-terminal-copy-fixes`](reports/launcher-and-terminal-copy-fixes.report.md) landed 2026-10-11 in #1908: fix the launcher's retry key and template-list errors, and copy in restored terminal snapshots
- [`supervisor-lifecycle-test-fixes`](reports/supervisor-lifecycle-test-fixes.report.md) landed 2026-10-11 in #1910: make seven supervisor lifecycle tests clean up their fixtures and prove the paths they name
- [`e2e-session-test-fixes`](reports/e2e-session-test-fixes.report.md) landed 2026-10-11 in #1911: make seven end-to-end terminal and session tests prove what they claim
