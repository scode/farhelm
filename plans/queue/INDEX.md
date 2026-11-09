# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [pending] `sandbox-sessions.md` — Launch a session in a named Tensorlake sandbox of its own from a recipe (launcher, templates, Clone), with explicit Suspend/Resume, Stop and suspend, Delete removing the sandbox, and agent operations refused (after `prepared-snapshots.md`)
- [in-flight 7ed931] `stop-start-message.md` — A Stop while the agent is still starting shows "Exited — stopped before the agent finished starting" instead of a launch failure blaming systemd; Restart and Replace get the same fix
- [in-flight 0b9109] `rail-notification-bell.md` — A notification bell in the rail for notices that are not about one session, red with a count when unread; its first notice is a trashed checkout whose files could not be freed, with Retry now, and the Trash dialog keeps such folders listed
- [in-flight 0be972] `settings-dialog-fixed-size.md` — The Settings dialog keeps one fixed height across sections (about 640px, capped by the window), its body scrolling for taller sections
- [in-flight 5dfb96] `upgrade-epochs.md` — Upgrade epochs as an upgrade floor: installations and hosts below the floor are told to uninstall and reinstall; the SPEC.md freeze becomes the floor rule
- [landing 8f79b4] `pin-rust-toolchain.md` — Pin Rust to the newest stable in rust-toolchain.toml so local checks, CI and release builds agree; bumps are manual PRs
- [complete] `xhigh-effort-label.md` — The extra-high effort level reads "xhigh" instead of "Xhigh" wherever effort labels show
- [complete] `scoped-stop-without-sweep.md` — Where a launch or tab runs in a systemd scope, Stop, tab close and Delete end the scope and skip the machine-wide process sweep; SPEC.md states what is no longer caught
- [pending] `session-switch-jiggle.md` — Find and fix what makes rightmost-column text and full-width rules jiggle in new terminals when switching sessions, with a browser test
