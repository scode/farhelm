# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`repo-clone-cache`](reports/repo-clone-cache.report.md) landed 2026-10-09 in #1720: Fresh GitHub checkouts go through a per-host repository cache in the supervisor's state directory, so only new objects cross the network; unused caches are removed after 30 days.
- [`row-marks-desktop-utf8`](reports/row-marks-desktop-utf8.report.md) landed 2026-10-09 in #1724: The desktop app declares UTF-8 for its page and stylesheet, so a session row with a notification bell stops drawing garbled characters over its agent marks.
- [`conversation-notice-hook-restart`](reports/conversation-notice-hook-restart.report.md) landed 2026-10-09 in #1722, #1723, #1725: The "conversation not learned" notification says, per agent, when the agent normally reports and what to do; an Enter answering a dialog no longer starts its clock; a restarted supervisor keeps checking the sessions it picks up.
