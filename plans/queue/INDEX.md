# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `unlikely-races-nov4.md` — terminals: fix four very unlikely races where straightforward, accept the rest in FILTER.md
- [complete] `durability-revocations.md` — power loss: write host permission revocations durably, key retries by host entry, and spec the accepted losses
- [complete] `delete-unreadable-terminals.md` — Delete: keep warning about and reaping work after a terminal stops responding
- [pending] `terminal-text-drops.md` — new terminals: state the invisible-formatting drop, print Kirat Rai vowel signs again
- [complete] `triage-misc-nov4.md` — template card shows the container, escape host text in helm logs, paste-after-gap clause, one discard
- [in-flight 21354e] `pinned-session-order.md` — Pinned sessions keep one shared manual order: new pins go to the bottom, and the user rearranges them by dragging or with Move up / Move down in the row menu
- [pending] `bulk-session-actions.md` — Shift+click and Cmd/Ctrl+click select several sessions; a bar offers Stop, Delete, Pin/Unpin and Mark read/unread, with one structured confirmation for a bulk Delete
- [in-flight 641e3a] `host-attention-list.md` — A list above the sessions shows every host with a problem, an update, or setup/update in progress, with its action; it replaces the update banner and card
- [pending] `host-filter-feedback.md` — A host chosen in the rail shows as a removable chip beside the status filter, and an empty filtered list names its host and status
- [pending] `tensorlake-ui-fixes.md` — Tensorlake fixes: recipe Save always visible, Suspending…/Resuming… states, launch-resume progress instead of a false "unavailable", Refresh with feedback, structured add-host and suspend dialogs
