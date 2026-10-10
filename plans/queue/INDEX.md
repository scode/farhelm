# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.
- [blocked] `waiting-sound.md` — sounds when a session waits, an approval arrives, or (opt-in) a turn finishes, each switchable per device in Settings
- [landing 88c8ce] `terminal-file-download.md` — path-shaped text in a terminal is checked on hover and downloads from the session's host on click (100 MB limit; desktop saves to Downloads)
- [blocked] `hook-report-watch.md` — the supervisor applies conversation reports as soon as they are written, via a file watch, with the timer as backstop (after `sweep-on-timer.md`)
- [complete] `managed-checkout-trash.md` — fresh GitHub checkouts become "managed checkouts": a branch glyph on their sessions, a folder/managed-checkout choice in the launcher and templates, and a trash beside New that lists archived checkouts per host and deletes them
- [blocked] `omp-pane-guard-all-launches.md` — the OMP Resume ownership check refuses an unrecognised Bun or Node pane for every launch type, not only installed `omp`
