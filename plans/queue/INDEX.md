# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 05f015] `triage-gui-misc.md` — execute the 2026-11-21 triage outcomes for the desktop app's launcher, templates, settings, Hosts panel and terminal view, in grouped PRs
- [complete] `triage-supervisor.md` — execute the 2026-11-21 triage outcomes for the supervisor (session lifecycle, shell sessions, cleanup), in grouped PRs
- [in-flight d98641] `triage-tensorlake.md` — execute the 2026-11-21 triage outcomes for Tensorlake sandboxes, in grouped PRs
- [pending] `launch-wakes-suspended-host.md` — make one press of Launch resume (or start) a suspended or stopped sandbox or container and create the session, as the browser app did
- [in-flight 442cdc] `recipe-credentials-in-launch-env.md` — give recipe Claude and Codex credentials to sessions through a Farhelm-owned file and the launch environment instead of editing the agents' settings files, with a protocol bump (after `agent-hook-setup.md`)
- [pending] `agent-trust-every-folder.md` — make the Claude, Codex, Muse and Pi trust switches apply in every folder, after checking the current agents' trust mechanisms live (after `agent-hook-setup.md`)
- [in-flight f109b6] `maximized-window-no-zoom-animation.md` — open a Mac window that was maximized at quit already zoomed, without the grow animation, keeping the ordinary frame a later zoom returns to
- [pending] `sqlite-compaction-settings.md` — add light upkeep to the helm and supervisor databases (a WAL size cap, planner statistics and a size log at owning opens, a checkpoint on orderly stop), with no vacuum
- [pending] `deflake-historical-base-prereqs.md` — refuse deflake sweeps of bases older than the current battery list, with an explanation, before anything starts
- [pending] `test-worker-staging-lock.md` — stage each test worker run separately and install it only under the lock, tie evidence to one invocation, add collect, and keep worker setup away from running tests
- [pending] `pace-bulk-metadata-changes.md` — raise the helm's limit on queued window commands (derived from the 500-row listing cap) so a metadata change to many marked sessions no longer resets the window
- [pending] `bulk-actions-bound-to-drawn.md` — make the marked bar's Stop and Delete act on the sessions and Skip session confirmations setting the bar showed, as the single-row menu does
- [pending] `interrupted-upload-notices.md` — post a notice in the notice center when the open session is no longer the one a file was still uploading to, so an interrupted upload is never lost unseen
- [pending] `wide-glyph-padding-cells.md` — keep a program's background color where a wide character wraps or is half erased in a terminal, marking wide-wrap padding in terminal rows, with a protocol bump
- [pending] `launcher-managed-checkout-compact.md` — make the launcher's managed-checkout destination compact: one Host | Repository address line, then the clone's folder and a one-line note across the full width
- [pending] `helpers-desktop-startup.md` — consolidate the desktop app's duplicated startup and updater helpers (reopen wait, hello probe, window size, stable-version check, state-file wiring, peer text, reinstall hover) without changing behavior
- [pending] `helpers-terminal-and-controls.md` — consolidate duplicated terminal input mapping, keypad digit lookup, control focus rings, focus checks, trash animation and control test helpers without changing behavior
- [pending] `helpers-hosts-and-sessions.md` — consolidate duplicated helpers in the Hosts panel, approvals, Settings, the session list and the session pane without changing behavior (after `triage-gui-misc.md`)
- [pending] `helpers-launcher-and-templates.md` — consolidate duplicated launcher, template and golden-scene fixture helpers without changing behavior (after `triage-gui-misc.md`, `launch-wakes-suspended-host.md`)
