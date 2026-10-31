# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `sqlite-wal.md` — The supervisor's and the helm's SQLite databases switch to WAL with synchronous NORMAL, converted in place by the process that owns them.
- [complete] `tab-output-hangup.md` — Find what terminated a heavy-output command in a live terminal tab and fix it; a tab is closed or signalled only when its shell really exited or the user closed or deleted it, with logs that name the tab and what was signalled.
- [in-flight 1898f8] `command-shell-meaning.md` — Command launches' commands, resume commands and wrappers run through bash with full shell meaning (so `~`, `$VAR` and globs work), host and container alike; breaking change.
