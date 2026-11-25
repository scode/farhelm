# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `drift-initial-prompt-launch.md` — Add live launch-time initial prompt delivery drift tests across supporting agents (after `drift-model-selection.md`)
- [complete] `drift-named-conversation-starts.md` — Add live one-time named conversation start drift tests across all built-in agents (after `drift-model-selection.md`)
- [in-flight ed6024] `drift-capture-timing-hooks.md` — Add live conversation capture timing, hook payload, and hooks-disabled drift tests (after `drift-model-selection.md`)
- [in-flight a5d53a] `drift-workspace-trust.md` — Add live workspace trust pre-trust drift tests across supporting agents (after `drift-model-selection.md`)
- [in-flight 99b304] `drift-instructions-delivery.md` — Add live `$farhelm` instructions pointer delivery drift tests across supporting agents (after `drift-model-selection.md`)
- [pending] `triage-bookkeeping-nov24.md` — execute the 50 discard outcomes from the 2026-11-24 triage
- [pending] `triage-spec-text-nov24.md` — write the 2026-11-24 triage's text corrections and new spec principles into SPEC.md, SPEC_impl.md, docs and comments
- [pending] `triage-tooling-nov24.md` — execute the 2026-11-24 triage outcomes for maintainer tooling and test infrastructure, in grouped PRs
- [pending] `triage-custom-agents-nov24.md` — execute the 2026-11-24 triage outcomes for registered custom agents, in grouped PRs
- [pending] `triage-launcher-nov24.md` — execute the 2026-11-24 triage outcomes for the session launcher and templates, in grouped PRs
- [pending] `triage-settings-nov24.md` — execute the 2026-11-24 triage outcomes for the Settings dialog, in grouped PRs
- [pending] `triage-sessions-ui-nov24.md` — execute the 2026-11-24 triage outcomes for the session list, notices and window chrome, in grouped PRs
- [pending] `triage-terminal-nov24.md` — execute the 2026-11-24 triage outcomes for the terminal view and emulation, in grouped PRs
- [pending] `triage-supervisor-nov24.md` — execute the 2026-11-24 triage outcomes for the supervisor's session lifecycle, in grouped PRs
- [pending] `triage-hooks-trust-nov24.md` — execute the 2026-11-24 triage outcomes for agent hooks and workspace trust, in grouped PRs
- [pending] `triage-helm-nov24.md` — execute the 2026-11-24 triage outcomes for hosts, setup, updates and agent requests, in grouped PRs
- [pending] `triage-podman-nov24.md` — execute the 2026-11-24 triage outcomes for containers (Podman), in grouped PRs
- [pending] `triage-environments-nov24.md` — execute the 2026-11-24 triage outcomes for sandboxes, waking, suspend and Trash, in grouped PRs
