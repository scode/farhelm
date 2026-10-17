# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`replace-retry-identity`](reports/replace-retry-identity.report.md) landed 2026-10-17 in #2205, #2206: Replace retries: one retry identity per source, and a fresh attempt after a definite refusal (2 outcomes, each gated)
