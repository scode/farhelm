# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`trash-confirm-binds-answer`](reports/trash-confirm-binds-answer.report.md) landed 2026-10-16 in #2171: Trash dialog: "delete permanently" deletes only the confirmation it was shown for, never a newer selection (1 outcome)
