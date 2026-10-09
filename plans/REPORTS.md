# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`upload-stall-same-session`](reports/upload-stall-same-session.report.md) landed 2026-10-09 in #1728: Payload uploads report the remote file's size on their own ssh session, so stall detection works on hosts that allow one session per connection.
- [`version-hover-text`](reports/version-hover-text.report.md) landed 2026-10-09 in #1733: Hovering the version readout says in plain words which Farhelm is running and, in the Mac app, that it turns red once an update is installed and can then be selected to restart into it.
- [`enter-launches-anywhere`](reports/enter-launches-anywhere.report.md) landed 2026-10-09 in #1729, #1735, #1737: Enter on a choice in the New session and Restart with dialogs chooses it and launches or restarts; a launch that cannot happen says why next to the Launch button.
