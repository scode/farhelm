# Plans queue

One line per plan, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. This file is excluded from dprint so a line is never rewrapped.

- [in-flight ae2f9a] `identity-report-wait-retry.md` — stop losing conversation-identity reports: no time limit on the supervisor's lock wait, Claude's sender check before it, a 30 s hook budget with retries across a supervisor restart.
- [pending] `host-update-button.md` — an amber or red outlined update button on hosts that run an older Farhelm, in place of the `old version` / `needs update` word (after `host-dialogs-and-menu.md`)
- [in-flight 3b42fb] `triage-crash-replies-and-locks.md` — a crashed Stop or Restart answers with an error, the per-host write lock survives a worker restart, and two systemd findings leave the queue.
- [in-flight 2639ad] `yolo-icon.md` — honest permission marks (slashed shield for YOLO, green shield for composer launches that ask, amber question mark otherwise), one shared YOLO classifier, OpenCode/OMP/Goose defaulting to YOLO like Pi, and the internal "sensitive host" names renamed to the user-facing ones.
- [in-flight 01618e] `desktop-internal-helm.md` — the desktop app's helm becomes internal: a random loopback port, no browser UI or token sign-in, only the app's own per-launch credentials, the `dioxus://` exemption kept only there, and the window's credential held in memory.
- [pending] `host-confirmations-toggle.md` — a gear beside the version number opens a settings dialog whose two checkboxes turn the host setup and removal confirmations back on (after `host-dialogs-and-menu.md`)
- [pending] `fresh-checkout-provenance.md` — the fresh-checkout replay e2e test checks the settings refusal again after #1455's random connection numbers, and a host's session list no longer fails to refresh when Replace with deletes a session that has its own checkout.
- [pending] `install-output-layout.md` — the installer supports only a Mac installing the desktop app (refusing Linux, explicitly not limiting Linux helms or hosts), drops two options, refuses releases without the app, and prints a few short formatted lines with download progress.
