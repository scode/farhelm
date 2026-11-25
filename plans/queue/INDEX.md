# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight ecd716] `triage-settings-nov24.md` — execute the 2026-11-24 triage outcomes for the Settings dialog, in grouped PRs
- [in-flight 097e88] `launcher-style-whats-new.md` — Give What's new the session launcher's look, every entry in full, with release curation marking each highlight's headline (after `launcher-style-quick-dialogs.md`)
- [pending] `welcome-empty-state.md` — Replace the startup Welcome card with a welcome in the empty main area, shown whenever there are no sessions (after `launcher-style-quick-dialogs.md`)
- [in-flight 8dab48] `launcher-consent-and-custom-agent-fixes.md` — Carry one-time YOLO consent through host wake, guard hook-setup continuation against late-arriving folders, distinguish unread custom agents from deleted ones, and show Custom approval flags on keyboard-highlighted recent setups
- [pending] `hooks-restart-and-antigravity-fixes.md` — Place Antigravity Restart options before Custom `--`, isolate hook setup errors in Restart and recovery UI, and delay the missing-hook warning clock until agent handoff (after `drop-post-clone-claude-trust.md`)
- [landing 13cfa0] `host-provisioning-action-fixes.md` — Pin a confirmed Tailscale Uninstall to the verified peer and give explicit host actions priority over automatic SSH sign-in
- [complete] `supervisor-stop-and-restart-cleanup-fixes.md` — Settle unconfirmed agent starts before Stop reports success and keep failed Restart cleanup retryable after session deletion
- [pending] `golden-harness-and-new-container-scene.md` — Report missing renders on the portable golden changes page and show the settled build status in the New container golden
- [pending] `agent-drift-and-screen-capture-tooling.md` — Stop agent updater process groups before drift cases, publish captured agent screens as recoverable pairs, and require a committed hook report after Restart in drift checks
- [pending] `brick-test-reopen-and-retry-fixes.md` — Keep the brick test's installer pair retryable after an unavailable P0 check and bind reopen samples to a witnessed app relaunch
