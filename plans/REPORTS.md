# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`osc8-bracketed-url-warnings`](reports/osc8-bracketed-url-warnings.report.md) landed 2026-11-24 in #4746, #4748: Restore terminal hyperlink mismatch warnings for nested bracketed and quoted URLs
- [`light-mode-upgrade-pill-pulse`](reports/light-mode-upgrade-pill-pulse.report.md) landed 2026-11-24 in #4747, #4750: Make the light-mode pending-update pill reminder pulse visible and bold its arrow in both themes
- [`preserve-creds-on-lookup-failure`](reports/preserve-creds-on-lookup-failure.report.md) landed 2026-11-24 in #4751, #4754: Preserve remote agent-credentials on SQLite lookup failure and name the affected agent
- [`custom-agents-cli-visibility`](reports/custom-agents-cli-visibility.report.md) landed 2026-11-24 in #4752, #4755, #4758: Add `farhelm agent agents` discovery, include custom agents in unknown-agent refusals, forbid `--conversation` with recipes, and list valid built-in efforts
- [`dynamic-announcements`](reports/dynamic-announcements.report.md) landed 2026-11-24 in #4749, #4753, #4756, #4759: Add a signed central announcements feed on get.farhelm.io with helm polling, bell notice, Settings toggle, and P2 brick test
- [`hide-non-actionable-file-hovers`](reports/hide-non-actionable-file-hovers.report.md) landed 2026-11-24 in #4777, #4779, #4780: Hide non-actionable terminal file hovers and wait silently while checking
- [`drift-model-selection`](reports/drift-model-selection.report.md) landed 2026-11-25 in #4781, #4782, #4783: Add drift test categories and live model selection coverage across supporting agents
