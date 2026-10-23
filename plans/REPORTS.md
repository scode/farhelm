# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`alias-in-add-host`](reports/alias-in-add-host.report.md) landed 2026-10-23 in #2666, #2667: The add host dialog takes an optional alias, written with the host as it is added; adding a destination that is already a host is refused, naming that host
- [`per-host-last-folder`](reports/per-host-last-folder.report.md) landed 2026-10-23 in #2669: The launcher remembers the folder of the last successful agent launch on each host and defaults New and quick launch to it, re-seeding an untouched folder when the host changes
- [`launch-refusal-visibility`](reports/launch-refusal-visibility.report.md) landed 2026-10-23 in #2675, #2677, #2678: A refused launch shows as a prominent alert by the launch button, a folder refusal marks the folder field, and the launcher warns before Launch when the folder does not exist on the host
- [`update-refuses-newer-host-schema`](reports/update-refuses-newer-host-schema.report.md) landed 2026-10-23 in #2649: Update refuses a host whose supervisor database is newer than the supervisor it would install (2026-10-22 triage)
- [`dialog-saves-oct22`](reports/dialog-saves-oct22.report.md) landed 2026-10-23 in #2651, #2658, #2679, #2661, #2663: Five dialog fixes: trash delete ignores clicks right after its list moves, Templates rechecks names, unknown shortcut saves say so, the commands help names deleting, unedited Settings Save writes nothing
- [`build-and-tooling-oct22`](reports/build-and-tooling-oct22.report.md) landed 2026-10-23 in #2657, #2659, #2660, #2662, #2665: Linux tmux terminfo path, plans queue ignores fork PRs, CentOS test keeps the ssh config, rerun installer repairs a damaged version folder, devbuild verifies the tmux signature (2026-10-22 triage)
