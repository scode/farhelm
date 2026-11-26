# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `session-right-click-menu.md` — Open session and bulk context menus on right-click (after `bulk-start-sessions.md`)
- [complete] `launcher-late-wake-failures.md` — Report request-tagged late wake failures for suspended sandboxes and stopped containers
- [in-flight 97032f] `podman-recipe-admission.md` — Report pre-admission container recipe Rebuild failures without creating empty build records (after `podman-host-verification.md`)
- [in-flight ae4852] `podman-proxy-free-images.md` — Keep build-time host proxy config out of captured Podman recipe images (after `podman-host-verification.md`)
- [complete] `supervisor-stop-pending-launch.md` — Refuse Stop on unconfirmed terminal-less launches in favor of Delete or Restart
- [landing 88717e] `agent-delete-late-notice.md` — Always record agent-Delete leftover-checkout warnings in the notices bell
- [complete] `drift-updater-teardown.md` — Run drift-test agent updaters in transient systemd user scopes for clean teardown
- [in-flight 184b35] `settings-wait-while-saving.md` — Make Settings Close/Escape and editors wait for saves, rip out slow-helm concurrency, and finish surviving Settings follow-ups (after `custom-agent-delete-prompt.md`)
- [in-flight 2a94e0] `terminal-tab-hover-close.md` — Move shell tab close button to a fixed left icon slot revealed on hover
- [pending] `agent-blocked-notifications.md` — Fix ntfy waiting alerts and add blocked-agent notifications with instruction injection (after `settings-wait-while-saving.md`)
