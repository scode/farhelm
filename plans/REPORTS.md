# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`template-name-escape`](reports/template-name-escape.report.md) landed 2026-10-17 in #2204: Template editor: an older template's unsafe session name shows escaped and read-only, like model, folder and commands
- [`replace-retry-identity`](reports/replace-retry-identity.report.md) landed 2026-10-17 in #2205, #2206: Replace retries: one retry identity per source, and a fresh attempt after a definite refusal (2 outcomes, each gated)
