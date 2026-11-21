# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `agent-hook-setup.md` — give every agent type its own Settings section, and let Grok and Antigravity users opt in once to Farhelm installing its Restart hooks into those agents on every machine, asked in the launcher and explained on the greyed Restart (after `launcher-command-field.md`)
- [pending] `triage-gui-misc.md` — execute the 2026-11-21 triage outcomes for the desktop app's launcher, templates, settings, Hosts panel and terminal view, in grouped PRs
- [pending] `triage-supervisor.md` — execute the 2026-11-21 triage outcomes for the supervisor (session lifecycle, shell sessions, cleanup), in grouped PRs
- [complete] `triage-helm.md` — execute the 2026-11-21 triage outcomes for the helm (hosts, provisioning, automatic update, desktop startup), in grouped PRs
- [complete] `triage-podman.md` — execute the 2026-11-21 triage outcomes for containers (Podman), in grouped PRs
- [pending] `triage-tensorlake.md` — execute the 2026-11-21 triage outcomes for Tensorlake sandboxes, in grouped PRs
- [in-flight 2f50cb] `environment-checkout-without-setup.md` — let a managed checkout in a session's own sandbox or container use the default checkout folder or else the machine's home folder, never asking or refusing over a missing folder
- [complete] `exited-terminal-scrollback.md` — let every retained terminal line be scrolled to and selected after an agent exits (or the red status line shows), including history under a killed full-screen agent
- [pending] `launch-wakes-suspended-host.md` — make one press of Launch resume (or start) a suspended or stopped sandbox or container and create the session, as the browser app did
- [pending] `recipe-credentials-in-launch-env.md` — give recipe Claude and Codex credentials to sessions through a Farhelm-owned file and the launch environment instead of editing the agents' settings files, with a protocol bump (after `agent-hook-setup.md`)
- [pending] `agent-trust-every-folder.md` — make the Claude, Codex, Muse and Pi trust switches apply in every folder, after checking the current agents' trust mechanisms live (after `agent-hook-setup.md`)
- [in-flight 2f272c] `status-mark-hover-ring.md` — stop drawing the hover ring around status icons (agent mark, idle time and the like) while controls keep it
