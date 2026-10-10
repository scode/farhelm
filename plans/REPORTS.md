# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`managed-checkout-trash`](reports/managed-checkout-trash.report.md) landed 2026-10-10 in #1785, #1786, #1787, #1788: fresh GitHub checkouts become "managed checkouts": a branch glyph on their sessions, a folder/managed-checkout choice in the launcher and templates, and a trash beside New that lists archived checkouts per host and deletes them
