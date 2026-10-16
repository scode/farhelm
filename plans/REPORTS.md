# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`trash-confirm-binds-answer`](reports/trash-confirm-binds-answer.report.md) landed 2026-10-16 in #2171: Trash dialog: "delete permanently" deletes only the confirmation it was shown for, never a newer selection (1 outcome)
- [`checkout-retry-helm-namespace`](reports/checkout-retry-helm-namespace.report.md) landed 2026-10-16 in #2172: Agent creates into managed checkouts: retry keys carry a per-helm-database namespace so a rebuilt registry cannot replay another host's approval (1 outcome)
- [`menu-arrows-follow-focus`](reports/menu-arrows-follow-focus.report.md) landed 2026-10-16 in #2176: Popup menus: arrow keys step from the item that actually has focus after a pointer or screen-reader focus move, keeping the fast-burst fix (1 outcome)
