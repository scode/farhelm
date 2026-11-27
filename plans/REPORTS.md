# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`shift-cmd-click-copy-file`](reports/shift-cmd-click-copy-file.report.md) landed 2026-11-27 in #5121, #5124: Add Shift-Cmd-click (Linux: Shift-Ctrl-click) on terminal file links to copy UTF-8 text or macOS image contents to the clipboard
- [`cli-effort-reject-flag-values`](reports/cli-effort-reject-flag-values.report.md) landed 2026-11-27 in #5129: Drop `allow_hyphen_values` on CLI `--effort` and `--model` so omitted values fail at parse time
- [`agent-host-tailscale-parity`](reports/agent-host-tailscale-parity.report.md) landed 2026-11-27 in #5126: Let agents pass `--tailscale` on `host add`, surface it on the approval card and dialog, and expose `tailscale` in `agent hosts --json`
- [`agent-blocked-notifications`](reports/agent-blocked-notifications.report.md) landed 2026-11-27 in #5118, #5125, #5135: Fix ntfy waiting alerts and add blocked-agent notifications with instruction injection
- [`claude-restart-before-first-turn`](reports/claude-restart-before-first-turn.report.md) landed 2026-11-27 in #5131, #5137, #5138: Restart a Claude session that saved nothing yet by starting it fresh instead of failing to resume
- [`settings-save-close-delete-prompt`](reports/settings-save-close-delete-prompt.report.md) landed 2026-11-27 in #5136: Document Settings Save-and-Close dismissing unanswered inline Delete confirmations and align Recipes' Back/Cancel leave prompt
- [`agent-delete-notices-bell-alignment`](reports/agent-delete-notices-bell-alignment.report.md) landed 2026-11-27 in #5140: Widen spec, docs, comments, and agent instructions from checkout notices to all agent-Delete cleanup notices
- [`marked-right-click-cancel-focus`](reports/marked-right-click-cancel-focus.report.md) landed 2026-11-27 in #5143: Return focus to the pre-right-click target when cancelling bulk Stop, Delete, or New group dialogs opened from the marked right-click menu
- [`stop-ambiguous-failed-restart-create`](reports/stop-ambiguous-failed-restart-create.report.md) landed 2026-11-27 in #5141: Keep uniform Stop refusal on unconfirmed launches and reword the refusal message accurately
- [`helm-replace-refuse-destination-change`](reports/helm-replace-refuse-destination-change.report.md) landed 2026-11-27 in #5142: Make the helm Replace API refuse destination folder changes and fresh managed checkouts
