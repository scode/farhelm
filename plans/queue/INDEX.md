# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `session-action-shortcuts.md` — keyboard shortcuts to delete and rename the open session (⌘W, ⌘L)
- [landing 196043] `quick-launch-chords.md` — compose quick launch templates with a ⌘X launch key sequence
- [in-flight 31e803] `tunnel-localhost-links.md` — open a remote session's localhost link through a tunnel in the Mac app
- [pending] `menu-focus-source.md` — menu arrows follow focus moved while arrow presses are landing
- [pending] `name-session-while-starting.md` — name a quick-launched session in a rename dialog while it starts (after `quick-launch-chords.md`)
