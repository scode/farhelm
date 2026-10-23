# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`alias-in-add-host`](reports/alias-in-add-host.report.md) landed 2026-10-23 in #2666, #2667: The add host dialog takes an optional alias, written with the host as it is added; adding a destination that is already a host is refused, naming that host
- [`build-and-tooling-oct22`](reports/build-and-tooling-oct22.report.md) landed 2026-10-23 in #2657, #2659, #2660, #2662, #2665: Linux tmux terminfo path, plans queue ignores fork PRs, CentOS test keeps the ssh config, rerun installer repairs a damaged version folder, devbuild verifies the tmux signature (2026-10-22 triage)
