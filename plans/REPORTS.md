# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`quick-switcher`](reports/quick-switcher.report.md) landed 2026-10-06 in #1691, #1692: Cmd+K (macOS) / Ctrl+Shift+K opens a Slack-style switcher: fuzzy-find any host's session and jump to it, open New with the typed name, or with `tl:` open New with a template applied.
