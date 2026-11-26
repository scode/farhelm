# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight ecd716] `triage-settings-nov24.md` — execute the 2026-11-24 triage outcomes for the Settings dialog, in grouped PRs
- [in-flight 2c74ef] `welcome-empty-state.md` — Replace the startup Welcome card with a welcome in the empty main area, shown whenever there are no sessions (after `launcher-style-quick-dialogs.md`)
- [in-flight 8dab48] `launcher-consent-and-custom-agent-fixes.md` — Carry one-time YOLO consent through host wake, guard hook-setup continuation against late-arriving folders, distinguish unread custom agents from deleted ones, and show Custom approval flags on keyboard-highlighted recent setups
- [pending] `hooks-restart-and-antigravity-fixes.md` — Place Antigravity Restart options before Custom `--`, isolate hook setup errors in Restart and recovery UI, and delay the missing-hook warning clock until agent handoff (after `drop-post-clone-claude-trust.md`)
- [in-flight 8bdd38] `agent-drift-and-screen-capture-tooling.md` — Stop agent updater process groups before drift cases, publish captured agent screens as recoverable pairs, and require a committed hook report after Restart in drift checks
- [pending] `brick-test-reopen-and-retry-fixes.md` — Keep the brick test's installer pair retryable after an unavailable P0 check and bind reopen samples to a witnessed app relaunch
