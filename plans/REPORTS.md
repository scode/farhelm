# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`triage-dialog-cancel-races`](reports/triage-dialog-cancel-races.report.md) landed 2026-10-05 in #1658, #1659, #1660, #1661, #1662, #1663: Cancelled YOLO questions and a closing feedback dialog stop acting on queued answers (complexity-gated).
- [`home-tab-trailing-slash`](reports/home-tab-trailing-slash.report.md) landed 2026-10-06 in #1673: A terminal tab beside a session in ~ starts in ~, not ~/ (strip the trailing slash handed to tmux).
- [`installer-feedback-prompt`](reports/installer-feedback-prompt.report.md) landed 2026-10-06 in #1674: A fresh install's closing message invites feedback, with the agreed wording.
- [`new-session-shortcut`](reports/new-session-shortcut.report.md) landed 2026-10-06 in #1675: Cmd+N opens a new session in the Mac desktop app.
