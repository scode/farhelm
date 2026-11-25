# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`osc8-bracketed-url-warnings`](reports/osc8-bracketed-url-warnings.report.md) landed 2026-11-24 in #4746, #4748: Restore terminal hyperlink mismatch warnings for nested bracketed and quoted URLs
- [`light-mode-upgrade-pill-pulse`](reports/light-mode-upgrade-pill-pulse.report.md) landed 2026-11-24 in #4747, #4750: Make the light-mode pending-update pill reminder pulse visible and bold its arrow in both themes
- [`preserve-creds-on-lookup-failure`](reports/preserve-creds-on-lookup-failure.report.md) landed 2026-11-24 in #4751, #4754: Preserve remote agent-credentials on SQLite lookup failure and name the affected agent
- [`custom-agents-cli-visibility`](reports/custom-agents-cli-visibility.report.md) landed 2026-11-24 in #4752, #4755, #4758: Add `farhelm agent agents` discovery, include custom agents in unknown-agent refusals, forbid `--conversation` with recipes, and list valid built-in efforts
- [`dynamic-announcements`](reports/dynamic-announcements.report.md) landed 2026-11-24 in #4749, #4753, #4756, #4759: Add a signed central announcements feed on get.farhelm.io with helm polling, bell notice, Settings toggle, and P2 brick test
- [`hide-non-actionable-file-hovers`](reports/hide-non-actionable-file-hovers.report.md) landed 2026-11-24 in #4777, #4779, #4780: Hide non-actionable terminal file hovers and wait silently while checking
- [`drift-model-selection`](reports/drift-model-selection.report.md) landed 2026-11-25 in #4781, #4782, #4783: Add drift test categories and live model selection coverage across supporting agents
- [`drift-reasoning-effort`](reports/drift-reasoning-effort.report.md) landed 2026-11-25 in #4785, #4791: Add live reasoning effort selection drift tests across supporting agents
- [`drift-permission-modes`](reports/drift-permission-modes.report.md) landed 2026-11-25 in #4784, #4792: Add live permission mode and custom permission flag drift tests across supporting agents
- [`drift-initial-prompt-launch`](reports/drift-initial-prompt-launch.report.md) landed 2026-11-25 in #4787, #4795: Add live launch-time initial prompt delivery drift tests across supporting agents
- [`drift-named-conversation-starts`](reports/drift-named-conversation-starts.report.md) landed 2026-11-25 in #4786, #4796: Add live one-time named conversation start drift tests across all built-in agents
- [`drift-capture-timing-hooks`](reports/drift-capture-timing-hooks.report.md) landed 2026-11-25 in #4797, #4806: Add live conversation capture timing, hook payload, and hooks-disabled drift tests
- [`drift-workspace-trust`](reports/drift-workspace-trust.report.md) landed 2026-11-25 in #4794, #4807: Add live workspace trust pre-trust drift tests across supporting agents
- [`triage-bookkeeping-nov24`](reports/triage-bookkeeping-nov24.report.md) landed 2026-11-25 in #4798: execute the 50 discard outcomes from the 2026-11-24 triage
- [`drift-instructions-delivery`](reports/drift-instructions-delivery.report.md) landed 2026-11-25 in #4804: Add live `$farhelm` instructions pointer delivery drift tests across supporting agents
- [`triage-spec-text-nov24`](reports/triage-spec-text-nov24.report.md) landed 2026-11-25 in #4799, #4800, #4802, #4805: write the 2026-11-24 triage's text corrections and new spec principles into SPEC.md, SPEC_impl.md, docs and comments
- [`triage-tooling-nov24`](reports/triage-tooling-nov24.report.md) landed 2026-11-25 in #4801, #4809, #4818, #4821, #4825, #4828, #4833: execute the 2026-11-24 triage outcomes for maintainer tooling and test infrastructure, in grouped PRs
- [`triage-sessions-ui-nov24`](reports/triage-sessions-ui-nov24.report.md) landed 2026-11-25 in #4811, #4813, #4816, #4822, #4826: execute the 2026-11-24 triage outcomes for the session list, notices and window chrome, in grouped PRs
- [`triage-launcher-nov24`](reports/triage-launcher-nov24.report.md) landed 2026-11-25 in #4812, #4814, #4819, #4823, #4829, #4832, #4836, #4839, #4841, #4843, #4847, #4850, #4856: execute the 2026-11-24 triage outcomes for the session launcher and templates, in grouped PRs
- [`triage-terminal-nov24`](reports/triage-terminal-nov24.report.md) landed 2026-11-25 in #4831, #4837, #4845, #4848, #4851, #4855, #4862, #4870: execute the 2026-11-24 triage outcomes for the terminal view and emulation, in grouped PRs
- [`triage-helm-nov24`](reports/triage-helm-nov24.report.md) landed 2026-11-25 in #4840, #4842, #4844, #4849, #4852, #4854, #4858, #4860, #4861, #4865, #4867, #4869, #4873, #4876, #4882: execute the 2026-11-24 triage outcomes for hosts, setup, updates and agent requests, in grouped PRs
