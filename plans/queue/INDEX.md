# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [pending] `update-refuses-newer-host-schema.md` — Update refuses a host whose supervisor database is newer than the supervisor it would install (2026-10-22 triage)
- [pending] `dialog-saves-oct22.md` — Five dialog fixes: trash delete ignores clicks right after its list moves, Templates rechecks names, unknown shortcut saves say so, the commands help names deleting, unedited Settings Save writes nothing (after `sign-in-preservation-sweep.md`)
- [blocked] `build-and-tooling-oct22.md` — Linux tmux terminfo path, plans queue ignores fork PRs, CentOS test keeps the ssh config, rerun installer repairs a damaged version folder, devbuild verifies the tmux signature (2026-10-22 triage)
- [in-flight 441161] `launch-refusal-visibility.md` — A refused launch shows as a prominent alert by the launch button, a folder refusal marks the folder field, and the launcher warns before Launch when the folder does not exist on the host
- [in-flight bc838d] `agent-add-host.md` — Agents can ask to add a host (opens the prefilled add host dialog) and to edit a host's alias, icon or color or remove a host (Allow/Deny cards); never approved automatically (after `alias-in-add-host.md`)
- [pending] `restart-with-as-launcher.md` — Restart with becomes a mode of the session launcher, with agent type, host and folder held fixed and templates offered as in Replace with; the separate dialog goes away (after `session-view-fixes-oct22.md`, `resume-target-fixes-oct22.md`, `launch-refusal-visibility.md`, `per-host-last-folder.md`)
