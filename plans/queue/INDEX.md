# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.
- [in-flight ead7df] `waiting-sound.md` — sounds when a session waits, an approval arrives, or (opt-in) a turn finishes, each switchable per device in Settings
- [in-flight 9da4fb] `terminal-file-download.md` — path-shaped text in a terminal is checked on hover and downloads from the session's host on click (100 MB limit; desktop saves to Downloads)
- [in-flight 8a8cab] `hook-report-watch.md` — the supervisor applies conversation reports as soon as they are written, via a file watch, with the timer as backstop (after `sweep-on-timer.md`)
- [complete] `omp-pane-guard-all-launches.md` — the OMP Resume ownership check refuses an unrecognised Bun or Node pane for every launch type, not only installed `omp`
