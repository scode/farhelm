# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 3436d4] `agent-blocked-notifications.md` — Fix ntfy waiting alerts and add blocked-agent notifications with instruction injection (after `settings-wait-while-saving.md`)
- [landing 824784] `agent-host-tailscale-parity.md` — Let agents pass `--tailscale` on `host add`, surface it on the approval card and dialog, and expose `tailscale` in `agent hosts --json`
- [landing d4b5b0] `codex-fork-and-vscode-resume.md` — Accept Codex `fork` SessionStart and `vscode` root transcript header sources in two stacked PRs so those sessions become resumable
- [in-flight e5077d] `claude-restart-before-first-turn.md` — Restart a Claude session that saved nothing yet by starting it fresh instead of failing to resume
- [complete] `cli-effort-reject-flag-values.md` — Drop `allow_hyphen_values` on CLI `--effort` and `--model` so omitted values fail at parse time
- [complete] `remove-legacy-sandbox-sessions-api.md` — Remove the unused legacy `POST /api/sandbox-sessions` helm endpoint and migrate its tests to `/api/environment-sessions`
- [complete] `restart-startup-scope-cleanup.md` — Clean the scope and shut down an unheld answering replacement host during Linux supervisor startup pending-scope cleanup
- [complete] `brick-test-inline-welcome.md` — Update macOS release brick-test instructions and unit tests for the inline Welcome view
- [complete] `claude-drift-trust-copy-back.md` — Add an atomic-rename regression test proving Claude drift copy-back never overwrites real ancestor trust
- [in-flight 70b6f3] `settings-save-close-delete-prompt.md` — Document Settings Save-and-Close dismissing unanswered inline Delete confirmations and align Recipes' Back/Cancel leave prompt
- [in-flight 3d1b8e] `helm-replace-refuse-destination-change.md` — Make the helm Replace API refuse destination folder changes and fresh managed checkouts
- [pending] `shell-tab-first-close-sweep-backoff.md` — Apply the existing 1 s to 60 s retry backoff when an exited shell tab's first automatic close sweep fails
- [pending] `agent-delete-notices-bell-alignment.md` — Widen spec, docs, comments, and agent instructions from checkout notices to all agent-Delete cleanup notices
- [pending] `marked-right-click-cancel-focus.md` — Return focus to the pre-right-click target when cancelling bulk Stop, Delete, or New group dialogs opened from the marked right-click menu
- [pending] `stop-ambiguous-failed-restart-create.md` — Keep uniform Stop refusal on unconfirmed launches and reword the refusal message accurately
