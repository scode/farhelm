# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight ecd716] `triage-settings-nov24.md` — execute the 2026-11-24 triage outcomes for the Settings dialog, in grouped PRs
- [pending] `launcher-style-quick-dialogs.md` — Give the quick switcher (Cmd+K), the template list (Cmd+J) and Rename (Cmd+L) the session launcher's look, with its pieces shared as components
- [in-flight b07ad9] `tensorlake-setup-and-repo-cache-fixes.md` — Isolate repository-cache startup sweep errors, refuse Tensorlake setup reruns on suspended or deleting sandboxes, and assert setup never reads agent settings
- [landing 4a7b03] `tailscale-private-socket-dirs.md` — Allocate private OS-chosen Tailscale SSH socket directories persisted across helm restarts and retire host masters on retarget and Remove without globbing `/tmp`
- [pending] `launcher-style-whats-new.md` — Give What's new the session launcher's look, every entry in full, with release curation marking each highlight's headline (after `launcher-style-quick-dialogs.md`)
- [pending] `welcome-empty-state.md` — Replace the startup Welcome card with a welcome in the empty main area, shown whenever there are no sessions (after `launcher-style-quick-dialogs.md`)
