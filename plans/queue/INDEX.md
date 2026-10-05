# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `agent-retry-by-request.md` — a keyed agent create, spawn or clone retried with the same request returns its session even after a template or source edit, with no time limit; the host compares the request as the agent sent it instead of what it resolved to, and the helm's 30-day stored-resolution table and its machinery are deleted; a keyed `agent create` must name `--host`; the key-reuse refusal stops echoing the internal key
- [in-flight d3eb4c] `get-site-cutover.md` — installs, helm payload downloads and the Mac app's updater switch to get.farhelm.io with signed checksums (installer and updater verified against a two-key ring, CI stops signing); PRs 1–4 of the cutover, README/docs switch excluded
