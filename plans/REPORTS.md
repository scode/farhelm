# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`checkout-retry-helm-namespace`](reports/checkout-retry-helm-namespace.report.md) landed 2026-10-16 in #2172: Agent creates into managed checkouts: retry keys carry a per-helm-database namespace so a rebuilt registry cannot replay another host's approval (1 outcome)
- [`menu-arrows-follow-focus`](reports/menu-arrows-follow-focus.report.md) landed 2026-10-16 in #2176: Popup menus: arrow keys step from the item that actually has focus after a pointer or screen-reader focus move, keeping the fast-burst fix (1 outcome)
- [`keyboard-scroll-regions`](reports/keyboard-scroll-regions.report.md) landed 2026-10-17 in #2190, #2191: Keyboard-reachable scroll areas: host setup and uninstall plans, and the trash list (2 outcomes, each gated)
- [`webkit-ime-guards`](reports/webkit-ime-guards.report.md) landed 2026-10-17 in #2194: Input methods in the Mac app and Safari: the key that ends a composition no longer submits or closes, across every guard
- [`download-cancel-pending-entry`](reports/download-cancel-pending-entry.report.md) landed 2026-10-17 in #2197: Cancelling a terminal download before it starts leaves no request bookkeeping behind (1 outcome, gated)
- [`triage-spec-rules-oct16`](reports/triage-spec-rules-oct16.report.md) landed 2026-10-17 in #2193, #2195, #2196: Spec rules from the 2026-10-16 triage: Claude lock trade-off, stale views in a second GUI, trash scale (3 outcomes, each gated)
- [`calm-session-unavailable`](reports/calm-session-unavailable.report.md) landed 2026-10-17 in #2198: Session pane: one calm "session unavailable" card instead of red and amber error text whenever the supervisor is out of reach
- [`bare-cache-explicit-git-dir`](reports/bare-cache-explicit-git-dir.report.md) landed 2026-10-17 in #2199: Fresh GitHub checkouts work on hosts with Git's explicit bare-repository policy (1 outcome, gated)
- [`claude-trust-startup-no-wait`](reports/claude-trust-startup-no-wait.report.md) landed 2026-10-17 in #2201: Supervisor startup no longer waits on Claude's settings lock for leftover trust cleanup (1 outcome, gated)
- [`agent-delete-and-prompt`](reports/agent-delete-and-prompt.report.md) landed 2026-10-17 in #2192, #2200, #2202, #2203: `farhelm agent delete` (approved, no self-delete), and an initial prompt at creation from the CLI, the launcher and templates
- [`template-name-escape`](reports/template-name-escape.report.md) landed 2026-10-17 in #2204: Template editor: an older template's unsafe session name shows escaped and read-only, like model, folder and commands
- [`replace-retry-identity`](reports/replace-retry-identity.report.md) landed 2026-10-17 in #2205, #2206: Replace retries: one retry identity per source, and a fresh attempt after a definite refusal (2 outcomes, each gated)
