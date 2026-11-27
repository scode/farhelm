# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`helm-replace-refuse-destination-change`](reports/helm-replace-refuse-destination-change.report.md) landed 2026-11-27 in #5142: Make the helm Replace API refuse destination folder changes and fresh managed checkouts
