# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 0d4a6c] `agent-hook-setup.md` — give every agent type its own Settings section, and let Grok and Antigravity users opt in once to Farhelm installing its Restart hooks into those agents on every machine, asked in the launcher and explained on the greyed Restart (after `launcher-command-field.md`)
- [complete] `triage-spec-text.md` — write the accepted current behavior from the 2026-11-21 triage outcomes into SPEC.md and SPEC_impl.md
- [in-flight bd615f] `triage-gui-sessions.md` — execute the 2026-11-21 triage outcomes for the desktop app's session list, sidebar, attention list and notices, in grouped PRs
- [pending] `triage-gui-misc.md` — execute the 2026-11-21 triage outcomes for the desktop app's launcher, templates, settings, Hosts panel and terminal view, in grouped PRs
- [pending] `triage-supervisor.md` — execute the 2026-11-21 triage outcomes for the supervisor (session lifecycle, shell sessions, cleanup), in grouped PRs
- [in-flight 7aa0d2] `triage-terminal-host.md` — execute the 2026-11-21 triage outcomes for the terminal host and terminal emulation, in grouped PRs
- [pending] `triage-helm.md` — execute the 2026-11-21 triage outcomes for the helm (hosts, provisioning, automatic update, desktop startup), in grouped PRs
- [pending] `triage-podman.md` — execute the 2026-11-21 triage outcomes for containers (Podman), in grouped PRs
- [pending] `triage-tensorlake.md` — execute the 2026-11-21 triage outcomes for Tensorlake sandboxes, in grouped PRs
- [pending] `triage-tooling.md` — execute the 2026-11-21 triage outcomes for maintainer tooling and test infrastructure, in grouped PRs
