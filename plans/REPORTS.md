# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

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
- [`triage-supervisor-nov24`](reports/triage-supervisor-nov24.report.md) landed 2026-11-25 in #4868, #4874, #4875, #4878, #4879, #4881, #4887: execute the 2026-11-24 triage outcomes for the supervisor's session lifecycle, in grouped PRs
