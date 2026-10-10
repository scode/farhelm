# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`approval-card-layout`](reports/approval-card-layout.report.md) landed 2026-10-10 in #1760: the agent request card moves to the top of the main pane, compact, one request expanded at a time
- [`resizable-sidebar`](reports/resizable-sidebar.report.md) landed 2026-10-10 in #1759: drag the sidebar's edge to resize it (240–600px), remembered per device
- [`sweep-on-timer`](reports/sweep-on-timer.report.md) landed 2026-10-10 in #1758, #1761, #1766: the supervisor sweeps on its 2 s timer only, and stops re-reading settled stopped sessions
- [`shutdown-expiry-quiesce`](reports/shutdown-expiry-quiesce.report.md) landed 2026-10-10 in #1780: a supervisor stop that runs out of time still briefly tells tmux to stop sending output first; the spec stops further defenses against the old tmux crash unless it recurs on 3.7c or later
- [`host-icons`](reports/host-icons.report.md) landed 2026-10-10 in #1765, #1770, #1775: remote hosts get a chosen icon and color, shown everywhere a host is named; the host settings dialog is refreshed and the launcher gets a custom host picker
- [`claude-compaction-status`](reports/claude-compaction-status.report.md) landed 2026-10-10 in #1774: Claude sessions read as working while compacting; the capture tool gains a /compact scenario and Claude is re-captured
- [`uninstall-ends-tmux`](reports/uninstall-ends-tmux.report.md) landed 2026-10-10 in #1781: hosts-panel uninstall also ends the host's private tmux server, by reloading systemd before the stop; the spec drops the promise to keep it
- [`managed-checkout-trash`](reports/managed-checkout-trash.report.md) landed 2026-10-10 in #1785, #1786, #1787, #1788: fresh GitHub checkouts become "managed checkouts": a branch glyph on their sessions, a folder/managed-checkout choice in the launcher and templates, and a trash beside New that lists archived checkouts per host and deletes them
