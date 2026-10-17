# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 750f3e] `menu-focus-source.md` — menu arrows follow focus moved while arrow presses are landing
- [in-flight 14dadc] `name-session-while-starting.md` — name a quick-launched session in a rename dialog while it starts (after `quick-launch-chords.md`)
