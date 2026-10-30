# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `host-short-names.md` — Hosts get a short name (alias, else the first hostname label), shown under each rail icon, beside it when the rail expands on hover, and on session rows
- [complete] `codex-no-daemon.md` — Pass `--no-daemon` on every Codex launch so sessions never attach to Codex's shared background server, and document why in SPEC_impl.md.
