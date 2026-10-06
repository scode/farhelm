# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`templates-dialog-overhaul`](reports/templates-dialog-overhaul.report.md) landed 2026-10-06 in #1678, #1687, #1688: Redesign the Templates panel (list beside editor, only the fields a template sets, choices that follow the agent type, undo for delete) and have new templates carry their launch kind.
- [`remember-feedback-contact`](reports/remember-feedback-contact.report.md) landed 2026-10-06 in #1689: Send feedback shows a "Re-use for future feedback" checkbox (on by default) and remembers the contact in the helm's shared preferences after a successful send, so the next dialog starts prefilled in the desktop app and the web UI alike.
- [`resolve-stale-notifications`](reports/resolve-stale-notifications.report.md) landed 2026-10-06 in #1690: A session's "never reported which conversation" and "Restart can no longer resume" notifications mark themselves resolved (greyed, counted as read) once Restart can resume again; a problem that comes back re-opens the same notification as new.
- [`quick-switcher`](reports/quick-switcher.report.md) landed 2026-10-06 in #1691, #1692: Cmd+K (macOS) / Ctrl+Shift+K opens a Slack-style switcher: fuzzy-find any host's session and jump to it, open New with the typed name, or with `tl:` open New with a template applied.
