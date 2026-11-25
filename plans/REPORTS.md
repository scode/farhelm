# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`triage-hooks-trust-nov24`](reports/triage-hooks-trust-nov24.report.md) landed 2026-11-25 in #4857, #4859, #4863, #4864, #4866, #4871, #4872, #4877, #4895, #4900, #4902, #4905, #4908, #4911, #4914, #4926: execute the 2026-11-24 triage outcomes for agent hooks and workspace trust, in grouped PRs
- [`hide-unconfigured-podman-recipe-status`](reports/hide-unconfigured-podman-recipe-status.report.md) landed 2026-11-25 in #4934: Hide unrelated unready Podman status noise on multi-provider recipe cards and disable Rebuild on unusable shown places
- [`sandbox-auto-trust-and-claude-onboarding`](reports/sandbox-auto-trust-and-claude-onboarding.report.md) landed 2026-11-25 in #4943, #4955, #4958: Automatically suppress workspace trust prompts in per-session sandboxes/containers and skip Claude Code first-run setup on new recipe hosts
- [`triage-environments-nov24`](reports/triage-environments-nov24.report.md) landed 2026-11-25 in #4886, #4891, #4898, #4901, #4910, #4916, #4918, #4922, #4925, #4929, #4931, #4937, #4939, #4942, #4947, #4948, #4949, #4953, #4961: execute the 2026-11-24 triage outcomes for sandboxes, waking, suspend and Trash, in grouped PRs
