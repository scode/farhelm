# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`codex-recipe-credentials-at-launch`](reports/codex-recipe-credentials-at-launch.report.md) landed 2026-11-23 in #4679, #4680, #4693: Supply Codex recipe API endpoint credentials at launch instead of writing ~/.codex/config.toml
- [`eye-catching-pending-upgrade`](reports/eye-catching-pending-upgrade.report.md) landed 2026-11-23 in #4706, #4719, #4722: Highlight pending desktop updates with a tinted red pill and periodic reminder animation
- [`custom-agents`](reports/custom-agents.report.md) landed 2026-11-23 in #4723, #4727, #4731, #4732, #4733: Add user-registered custom agents, the custom-agent contract, CLI reporter, and docs
