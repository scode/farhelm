# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`triage-hooks-trust-nov24`](reports/triage-hooks-trust-nov24.report.md) landed 2026-11-25 in #4857, #4859, #4863, #4864, #4866, #4871, #4872, #4877, #4895, #4900, #4902, #4905, #4908, #4911, #4914, #4926: execute the 2026-11-24 triage outcomes for agent hooks and workspace trust, in grouped PRs
- [`triage-podman-nov24`](reports/triage-podman-nov24.report.md) landed 2026-11-25 in #4880, #4883, #4884, #4885, #4888, #4889, #4894, #4907, #4909, #4917, #4921, #4927, #4936: execute the 2026-11-24 triage outcomes for containers (Podman), in grouped PRs
- [`global-checkout-repo-history`](reports/global-checkout-repo-history.report.md) landed 2026-11-25 in #4928, #4938, #4944: Replace per-host Managed checkout repository suggestions and host directory scanning with one helm-global repository history
- [`hide-unconfigured-podman-recipe-status`](reports/hide-unconfigured-podman-recipe-status.report.md) landed 2026-11-25 in #4934: Hide unrelated unready Podman status noise on multi-provider recipe cards and disable Rebuild on unusable shown places
- [`tab-close-activation-order`](reports/tab-close-activation-order.report.md) landed 2026-11-25 in #4940: Activate the previous terminal tab in open order when closing an active shell tab
