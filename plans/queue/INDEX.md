# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 2b7c41] `bulk-start-sessions.md` — Add bulk Restart/Replace action for marked stopped sessions
- [pending] `session-right-click-menu.md` — Open session and bulk context menus on right-click (after `bulk-start-sessions.md`)
- [complete] `restart-with-generation-sync.md` — Bind Restart-with launch settings with the replacement generation before startup reports
- [in-flight 7ce017] `custom-agent-per-reg-wrappers.md` — Remember command wrapper defaults per custom agent registration
- [complete] `wayland-async-text-paste.md` — Make Wayland desktop text-field paste nonblocking and text-first
- [in-flight c93efa] `custom-agent-delete-prompt.md` — Port Settings custom-agent Delete to the shared inline confirmation prompt
- [pending] `tensorlake-sandbox-key-consent.md` — Bind Settings leftover-sandbox Delete confirmation and list to the active Tensorlake key
- [pending] `launcher-visible-consent.md` — Lock host and destination in Replace with and record launcher list Enter visibility rule
- [pending] `launcher-late-wake-failures.md` — Report request-tagged late wake failures for suspended sandboxes and stopped containers
- [pending] `podman-host-verification.md` — Verify container host identity with cheap sanity checks and record concurrent SSH config policy
- [pending] `podman-recipe-admission.md` — Report pre-admission container recipe Rebuild failures without creating empty build records (after `podman-host-verification.md`)
- [pending] `podman-proxy-free-images.md` — Keep build-time host proxy config out of captured Podman recipe images (after `podman-host-verification.md`)
- [pending] `supervisor-outliving-programs.md` — Root Stop and Restart cleanup at surviving pinned children and record SIGHUP/SIGKILL policy
- [pending] `supervisor-stop-pending-launch.md` — Refuse Stop on unconfirmed terminal-less launches in favor of Delete or Restart
- [pending] `supervisor-tab-close-retry.md` — Fix in-memory shell-tab close retry bugs and record Linux systemd/unscoped cleanup policy
- [pending] `agent-delete-late-notice.md` — Always record agent-Delete leftover-checkout warnings in the notices bell
- [pending] `drift-updater-teardown.md` — Run drift-test agent updaters in transient systemd user scopes for clean teardown
- [pending] `settings-wait-while-saving.md` — Make Settings Close/Escape and editors wait for saves, rip out slow-helm concurrency, and finish surviving Settings follow-ups (after `custom-agent-delete-prompt.md`)
