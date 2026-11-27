# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `agent-blocked-notifications.md` — Fix ntfy waiting alerts and add blocked-agent notifications with instruction injection (after `settings-wait-while-saving.md`)
- [complete] `claude-restart-before-first-turn.md` — Restart a Claude session that saved nothing yet by starting it fresh instead of failing to resume
- [complete] `remove-legacy-sandbox-sessions-api.md` — Remove the unused legacy `POST /api/sandbox-sessions` helm endpoint and migrate its tests to `/api/environment-sessions`
- [complete] `restart-startup-scope-cleanup.md` — Clean the scope and shut down an unheld answering replacement host during Linux supervisor startup pending-scope cleanup
- [complete] `brick-test-inline-welcome.md` — Update macOS release brick-test instructions and unit tests for the inline Welcome view
- [complete] `claude-drift-trust-copy-back.md` — Add an atomic-rename regression test proving Claude drift copy-back never overwrites real ancestor trust
- [complete] `settings-save-close-delete-prompt.md` — Document Settings Save-and-Close dismissing unanswered inline Delete confirmations and align Recipes' Back/Cancel leave prompt
- [in-flight 3d1b8e] `helm-replace-refuse-destination-change.md` — Make the helm Replace API refuse destination folder changes and fresh managed checkouts
- [complete] `shell-tab-first-close-sweep-backoff.md` — Apply the existing 1 s to 60 s retry backoff when an exited shell tab's first automatic close sweep fails
- [in-flight ced1a3] `agent-delete-notices-bell-alignment.md` — Widen spec, docs, comments, and agent instructions from checkout notices to all agent-Delete cleanup notices
- [in-flight 1c1472] `marked-right-click-cancel-focus.md` — Return focus to the pre-right-click target when cancelling bulk Stop, Delete, or New group dialogs opened from the marked right-click menu
- [in-flight f6c41a] `stop-ambiguous-failed-restart-create.md` — Keep uniform Stop refusal on unconfirmed launches and reword the refusal message accurately
