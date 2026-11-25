# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 27c7dc] `drift-capture-timing-hooks.md` — Add live conversation capture timing, hook payload, and hooks-disabled drift tests (after `drift-model-selection.md`)
- [complete] `drift-workspace-trust.md` — Add live workspace trust pre-trust drift tests across supporting agents (after `drift-model-selection.md`)
- [complete] `drift-instructions-delivery.md` — Add live `$farhelm` instructions pointer delivery drift tests across supporting agents (after `drift-model-selection.md`)
- [complete] `triage-bookkeeping-nov24.md` — execute the 50 discard outcomes from the 2026-11-24 triage
- [complete] `triage-spec-text-nov24.md` — write the 2026-11-24 triage's text corrections and new spec principles into SPEC.md, SPEC_impl.md, docs and comments
- [in-flight 66113d] `triage-tooling-nov24.md` — execute the 2026-11-24 triage outcomes for maintainer tooling and test infrastructure, in grouped PRs
- [in-flight 2b20d7] `triage-custom-agents-nov24.md` — execute the 2026-11-24 triage outcomes for registered custom agents, in grouped PRs
- [in-flight 03cb5b] `triage-launcher-nov24.md` — execute the 2026-11-24 triage outcomes for the session launcher and templates, in grouped PRs
- [pending] `triage-settings-nov24.md` — execute the 2026-11-24 triage outcomes for the Settings dialog, in grouped PRs
- [in-flight ca90c3] `triage-sessions-ui-nov24.md` — execute the 2026-11-24 triage outcomes for the session list, notices and window chrome, in grouped PRs
- [pending] `triage-terminal-nov24.md` — execute the 2026-11-24 triage outcomes for the terminal view and emulation, in grouped PRs
- [pending] `triage-supervisor-nov24.md` — execute the 2026-11-24 triage outcomes for the supervisor's session lifecycle, in grouped PRs
- [pending] `triage-hooks-trust-nov24.md` — execute the 2026-11-24 triage outcomes for agent hooks and workspace trust, in grouped PRs
- [pending] `triage-helm-nov24.md` — execute the 2026-11-24 triage outcomes for hosts, setup, updates and agent requests, in grouped PRs
- [pending] `triage-podman-nov24.md` — execute the 2026-11-24 triage outcomes for containers (Podman), in grouped PRs
- [pending] `triage-environments-nov24.md` — execute the 2026-11-24 triage outcomes for sandboxes, waking, suspend and Trash, in grouped PRs
- [pending] `sandbox-auto-trust-and-claude-onboarding.md` — Automatically suppress workspace trust prompts in per-session sandboxes/containers and skip Claude Code first-run setup on new recipe hosts
- [pending] `global-checkout-repo-history.md` — Replace per-host Managed checkout repository suggestions and host directory scanning with one helm-global repository history
- [pending] `hide-unconfigured-podman-recipe-status.md` — Hide unrelated unready Podman status noise on multi-provider recipe cards and disable Rebuild on unusable shown places
