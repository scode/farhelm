# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 28bf2e] `build-and-tooling-oct22.md` — Linux tmux terminfo path, plans queue ignores fork PRs, CentOS test keeps the ssh config, rerun installer repairs a damaged version folder, devbuild verifies the tmux signature (2026-10-22 triage)
- [complete] `agent-add-host.md` — Agents can ask to add a host (opens the prefilled add host dialog) and to edit a host's alias, icon or color or remove a host (Allow/Deny cards); never approved automatically (after `alias-in-add-host.md`)
- [complete] `restart-with-as-launcher.md` — Restart with becomes a mode of the session launcher, with agent type, host and folder held fixed and templates offered as in Replace with; the separate dialog goes away (after `session-view-fixes-oct22.md`, `resume-target-fixes-oct22.md`, `launch-refusal-visibility.md`, `per-host-last-folder.md`)
