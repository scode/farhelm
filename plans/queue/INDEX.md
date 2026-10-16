# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 6a9ece] `trash-confirm-binds-answer.md` — Trash dialog: "delete permanently" deletes only the confirmation it was shown for, never a newer selection (1 outcome)
- [blocked] `menu-arrows-follow-focus.md` — Popup menus: arrow keys step from the item that actually has focus after a pointer or screen-reader focus move, keeping the fast-burst fix (1 outcome)
- [complete] `checkout-retry-helm-namespace.md` — Agent creates into managed checkouts: retry keys carry a per-helm-database namespace so a rebuilt registry cannot replay another host's approval (1 outcome)
