# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 2b20d7] `triage-custom-agents-nov24.md` — execute the 2026-11-24 triage outcomes for registered custom agents, in grouped PRs
- [landing 0942ac] `triage-launcher-nov24.md` — execute the 2026-11-24 triage outcomes for the session launcher and templates, in grouped PRs
- [pending] `triage-settings-nov24.md` — execute the 2026-11-24 triage outcomes for the Settings dialog, in grouped PRs
- [in-flight 021290] `triage-terminal-nov24.md` — execute the 2026-11-24 triage outcomes for the terminal view and emulation, in grouped PRs
- [pending] `triage-supervisor-nov24.md` — execute the 2026-11-24 triage outcomes for the supervisor's session lifecycle, in grouped PRs
- [in-flight 00b06a] `triage-hooks-trust-nov24.md` — execute the 2026-11-24 triage outcomes for agent hooks and workspace trust, in grouped PRs
- [in-flight ee8dba] `triage-helm-nov24.md` — execute the 2026-11-24 triage outcomes for hosts, setup, updates and agent requests, in grouped PRs
- [pending] `triage-podman-nov24.md` — execute the 2026-11-24 triage outcomes for containers (Podman), in grouped PRs
- [pending] `triage-environments-nov24.md` — execute the 2026-11-24 triage outcomes for sandboxes, waking, suspend and Trash, in grouped PRs
- [pending] `sandbox-auto-trust-and-claude-onboarding.md` — Automatically suppress workspace trust prompts in per-session sandboxes/containers and skip Claude Code first-run setup on new recipe hosts
- [pending] `global-checkout-repo-history.md` — Replace per-host Managed checkout repository suggestions and host directory scanning with one helm-global repository history
- [pending] `hide-unconfigured-podman-recipe-status.md` — Hide unrelated unready Podman status noise on multi-provider recipe cards and disable Rebuild on unusable shown places
- [pending] `launcher-style-quick-dialogs.md` — Give the quick switcher (Cmd+K), the template list (Cmd+J) and Rename (Cmd+L) the session launcher's look, with its pieces shared as components
