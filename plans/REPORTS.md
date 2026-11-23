# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`claude-trust-home-dir-policy`](reports/claude-trust-home-dir-policy.report.md) landed 2026-11-23 in #4664, #4667: Skip Claude workspace trust in home, its parents, and root, fail closed on unusable HOME, and align Settings and docs
- [`tensorlake-lifecycle-sandbox-identity`](reports/tensorlake-lifecycle-sandbox-identity.report.md) landed 2026-11-23 in #4670: Refuse Tensorlake Suspend and Resume without a confirmed sandbox and persist recovered IDs under the provisioning lock
- [`stop-and-suspend-timeout-feedback`](reports/stop-and-suspend-timeout-feedback.report.md) landed 2026-11-23 in #4668, #4671, #4676: Distinguish Stop and suspend's 20-second reply notice from supervisor timeouts in helm and desktop app
