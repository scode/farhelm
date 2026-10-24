# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`agent-add-host`](reports/agent-add-host.report.md) landed 2026-10-23 in #2674, #2680, #2681: Agents can ask to add a host (opens the prefilled add host dialog) and to edit a host's alias, icon or color or remove a host (Allow/Deny cards); never approved automatically
- [`restart-with-as-launcher`](reports/restart-with-as-launcher.report.md) landed 2026-10-23 in #2682, #2683: Restart with becomes a mode of the session launcher, with agent type, host and folder held fixed and templates offered as in Replace with; the separate dialog goes away
- [`terminal-font-required`](reports/terminal-font-required.report.md) landed 2026-10-24 in #2700: terminals wait for the bundled JetBrains Mono font like their bundled scripts, with no 3 s deadline, fallback font, or late swap and repaint
