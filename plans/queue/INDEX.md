# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `fallback-unattachable-remembered-tab.md` — Fall back to the main terminal when a remembered shell tab receives an explicit initial attach refusal
- [complete] `fix-muse-conversation-reporting.md` — Pass non-secret session identity args to Muse Code hooks across environment scrubbing
- [in-flight 61b9d9] `button-restyle.md` — Restyle buttons and button-like controls without outlines, with the larger session toolbar
- [complete] `gui-goldens-in-releases.md` — Document keeping GPUI screenshot goldens development- and landing-only and remove the TODO entry
- [complete] `repo-cache-lock-redesign.md` — Replace repository cache Git lock deletion with atomic quarantine-and-rebuild
- [complete] `brick-test-remote-mac-uninstall.md` — Extend release brick test P1 with a Tart-managed remote Mac uninstall pass
- [in-flight aeef77] `remove-host-signin-lock.md` — Stop removing an SSH host from blocking on open sign-in prompts and probes
- [in-flight 0e5a96] `eye-catching-pending-upgrade.md` — Highlight pending desktop updates with a tinted red pill and periodic reminder animation
- [pending] `host-auto-update-after-2fa.md` — Prompt for SSH sign-in and resume automatic remote-host updates (after `remove-host-signin-lock.md`)
