# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`triage-trust-settings-and-ui-wording`](reports/triage-trust-settings-and-ui-wording.report.md) landed 2026-11-26 in #5043, #5046, #5049: Execute 7 triage outcomes for best-effort Custom auto-trust flags, unread Settings switch a11y, persistent-host trust help, disposable trust hints, Restart card trust copy, and trust test gaps
- [`triage-launcher-safety`](reports/triage-launcher-safety.report.md) landed 2026-11-26 in #5050, #5051: Execute 5 triage outcomes for Replace-with Recent screen-reader wording, New/Clone vanished-pick Enter guards, held Escape, stale permanent YOLO feedback, and repository-history load errors
- [`landing-readme-agent-records`](reports/landing-readme-agent-records.report.md) landed 2026-11-26 in #5058: Point the landing tooling README at `.agents/agent-records.md`
- [`wayland-async-text-paste`](reports/wayland-async-text-paste.report.md) landed 2026-11-26 in #5069: Make Wayland desktop text-field paste nonblocking and text-first
- [`restart-with-generation-sync`](reports/restart-with-generation-sync.report.md) landed 2026-11-26 in #5071: Bind Restart-with launch settings with the replacement generation before startup reports
- [`custom-agent-per-reg-wrappers`](reports/custom-agent-per-reg-wrappers.report.md) landed 2026-11-26 in #5073, #5074: Remember command wrapper defaults per custom agent registration
- [`bulk-start-sessions`](reports/bulk-start-sessions.report.md) landed 2026-11-26 in #5065, #5072: Add bulk Restart/Replace action for marked stopped sessions
- [`custom-agent-delete-prompt`](reports/custom-agent-delete-prompt.report.md) landed 2026-11-26 in #5075: Port Settings custom-agent Delete to the shared inline confirmation prompt
- [`tensorlake-sandbox-key-consent`](reports/tensorlake-sandbox-key-consent.report.md) landed 2026-11-26 in #5077: Bind Settings leftover-sandbox Delete confirmation and list to the active Tensorlake key
- [`launcher-visible-consent`](reports/launcher-visible-consent.report.md) landed 2026-11-26 in #5081, #5082: Lock host and destination in Replace with and record launcher list Enter visibility rule
- [`podman-host-verification`](reports/podman-host-verification.report.md) landed 2026-11-26 in #5084: Verify container host identity with cheap sanity checks and record concurrent SSH config policy
- [`supervisor-outliving-programs`](reports/supervisor-outliving-programs.report.md) landed 2026-11-26 in #5083: Root Stop and Restart cleanup at surviving pinned children and record SIGHUP/SIGKILL policy
- [`supervisor-tab-close-retry`](reports/supervisor-tab-close-retry.report.md) landed 2026-11-26 in #5087: Fix in-memory shell-tab close retry bugs and record Linux systemd/unscoped cleanup policy
- [`agent-delete-late-notice`](reports/agent-delete-late-notice.report.md) landed 2026-11-26 in #5088: Always record agent-Delete leftover-checkout warnings in the notices bell
- [`session-right-click-menu`](reports/session-right-click-menu.report.md) landed 2026-11-26 in #5090: Open session and bulk context menus on right-click
- [`launcher-late-wake-failures`](reports/launcher-late-wake-failures.report.md) landed 2026-11-26 in #5079, #5092: Report request-tagged late wake failures for suspended sandboxes and stopped containers
- [`supervisor-stop-pending-launch`](reports/supervisor-stop-pending-launch.report.md) landed 2026-11-26 in #5091: Refuse Stop on unconfirmed terminal-less launches in favor of Delete or Restart
- [`drift-updater-teardown`](reports/drift-updater-teardown.report.md) landed 2026-11-26 in #5093: Run drift-test agent updaters in transient systemd user scopes for clean teardown
- [`podman-recipe-admission`](reports/podman-recipe-admission.report.md) landed 2026-11-27 in #5094: Report pre-admission container recipe Rebuild failures without creating empty build records
- [`podman-proxy-free-images`](reports/podman-proxy-free-images.report.md) landed 2026-11-27 in #5095: Keep build-time host proxy config out of captured Podman recipe images
- [`settings-wait-while-saving`](reports/settings-wait-while-saving.report.md) landed 2026-11-27 in #5102, #5103, #5104, #5105, #5106: Make Settings Close/Escape and editors wait for saves, rip out slow-helm concurrency, and finish surviving Settings follow-ups
- [`terminal-tab-hover-close`](reports/terminal-tab-hover-close.report.md) landed 2026-11-27 in #5096: Move shell tab close button to a fixed left icon slot revealed on hover
