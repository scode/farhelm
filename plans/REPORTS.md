# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`settings-wait-while-saving`](reports/settings-wait-while-saving.report.md) landed 2026-11-27 in #5102, #5103, #5104, #5105, #5106: Make Settings Close/Escape and editors wait for saves, rip out slow-helm concurrency, and finish surviving Settings follow-ups
- [`terminal-tab-hover-close`](reports/terminal-tab-hover-close.report.md) landed 2026-11-27 in #5096: Move shell tab close button to a fixed left icon slot revealed on hover
