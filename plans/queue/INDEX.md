# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight d9a93f] `session-group-follow-ups.md` — Session groups: indent members, mark a collapsed group holding the open session, drag sessions into and out of groups
- [in-flight 8e15a2] `submenus-open-on-hover.md` — Submenus open on hover, keeping their parent item highlighted
- [in-flight d298d9] `clone-keeps-invisible-characters.md` — Launcher fields keep invisible characters when edited; the suspended-host refusal matches the button
- [in-flight eba2fc] `shared-placement-picker.md` — Shared placement picker drops its no-op press and moves to a shared module, with the template editor test
- [pending] `retire-old-terminal-announcement.md` — Remove the stale 0.37.0 old-terminal in-app announcement
- [pending] `quick-switcher-shot-paths.md` — Quick switcher shows staged folders in agent builds, so its screenshots pass the privacy check
- [pending] `model-families.md` — Known models with a latest model per family, replacing the retired-model special case
- [pending] `session-limit-notice.md` — Rail bell notice when a host or the helm hits the 500-session ceiling
