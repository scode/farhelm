# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing df071e] `helm-replace-refuse-destination-change.md` — Make the helm Replace API refuse destination folder changes and fresh managed checkouts
- [landing 4dabc6] `agent-delete-notices-bell-alignment.md` — Widen spec, docs, comments, and agent instructions from checkout notices to all agent-Delete cleanup notices
- [complete] `marked-right-click-cancel-focus.md` — Return focus to the pre-right-click target when cancelling bulk Stop, Delete, or New group dialogs opened from the marked right-click menu
- [complete] `stop-ambiguous-failed-restart-create.md` — Keep uniform Stop refusal on unconfirmed launches and reword the refusal message accurately
