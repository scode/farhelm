# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`claude-trust-home-dir-policy`](reports/claude-trust-home-dir-policy.report.md) landed 2026-11-23 in #4664, #4667: Skip Claude workspace trust in home, its parents, and root, fail closed on unusable HOME, and align Settings and docs
