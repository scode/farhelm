# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight f2b8da] `session-notifications.md` — session-tracking problems (no conversation identity after 65 s, hook not added, report refused, resume withdrawn) become persistent per-session notifications behind a bell on the sidebar row, with read-on-close and clear (after `launch-representation.md`, `hover-help.md`)
- [in-flight 5be030] `agent-retry-by-request.md` — a keyed agent create, spawn or clone retried with the same request returns its session even after a template or source edit, with no time limit; the host compares the request as the agent sent it instead of what it resolved to, and the helm's 30-day stored-resolution table and its machinery are deleted; a keyed `agent create` must name `--host`; the key-reuse refusal stops echoing the internal key
