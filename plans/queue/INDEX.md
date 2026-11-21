# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing aac53e] `templates-list-bolt-and-key.md` — center the ⚡ in the Templates list and show each template's number key in a fixed-width cap, with an empty cap for templates without one
- [complete] `menu-shortcut-hints.md` — show keyboard shortcuts as key chips in menus: ⌘L and ⌘W in the open session's menu, ⌘/ in Help
- [in-flight 9adac2] `whats-new-on-upgrade.md` — after an update, show the Mac app's release highlights since the last release shown, downloaded from a data file the website publishes, with a Settings switch and a release-procedure stop for deploying the website
- [in-flight 0332f4] `launcher-command-field.md` — make the session launcher's search field its header and starting point, with hints, a left column of recent setups, templates and results, a calmer form, and no footer summary
- [pending] `settings-dialog-refresh.md` — give Settings grouped sections with one hue per group, cards under a sticky page heading, a pinned footer Save for every form, and a question before closing with unsaved changes (after `whats-new-on-upgrade.md`)
