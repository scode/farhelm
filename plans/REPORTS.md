# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`gui-text-safety`](reports/gui-text-safety.report.md) landed 2026-10-10 in #1811: pastes lose hidden end-of-paste markers, template summaries and identity labels show hidden characters, and templates refuse commands with control or invisible characters
