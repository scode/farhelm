# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`upload-stall-same-session`](reports/upload-stall-same-session.report.md) landed 2026-10-09 in #1728: Payload uploads report the remote file's size on their own ssh session, so stall detection works on hosts that allow one session per connection.
