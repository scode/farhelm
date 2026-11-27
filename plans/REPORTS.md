# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`session-right-click-menu`](reports/session-right-click-menu.report.md) landed 2026-11-26 in #5090: Open session and bulk context menus on right-click
- [`launcher-late-wake-failures`](reports/launcher-late-wake-failures.report.md) landed 2026-11-26 in #5079, #5092: Report request-tagged late wake failures for suspended sandboxes and stopped containers
- [`supervisor-stop-pending-launch`](reports/supervisor-stop-pending-launch.report.md) landed 2026-11-26 in #5091: Refuse Stop on unconfirmed terminal-less launches in favor of Delete or Restart
- [`drift-updater-teardown`](reports/drift-updater-teardown.report.md) landed 2026-11-26 in #5093: Run drift-test agent updaters in transient systemd user scopes for clean teardown
- [`podman-recipe-admission`](reports/podman-recipe-admission.report.md) landed 2026-11-27 in #5094: Report pre-admission container recipe Rebuild failures without creating empty build records
- [`podman-proxy-free-images`](reports/podman-proxy-free-images.report.md) landed 2026-11-27 in #5095: Keep build-time host proxy config out of captured Podman recipe images
- [`settings-wait-while-saving`](reports/settings-wait-while-saving.report.md) landed 2026-11-27 in #5102, #5103, #5104, #5105, #5106: Make Settings Close/Escape and editors wait for saves, rip out slow-helm concurrency, and finish surviving Settings follow-ups
- [`terminal-tab-hover-close`](reports/terminal-tab-hover-close.report.md) landed 2026-11-27 in #5096: Move shell tab close button to a fixed left icon slot revealed on hover
