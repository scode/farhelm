# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `host-rail-list-visible.md` — The host rail no longer widens on hover; each host's tooltip carries its full name and destination, and a new button at the rail's foot expands the whole rail into a labeled host list beside the session list, remembered per device.
- [complete] `sqlite-wal.md` — The supervisor's and the helm's SQLite databases switch to WAL with synchronous NORMAL, converted in place by the process that owns them.
