# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`agent-retry-by-request`](reports/agent-retry-by-request.report.md) landed 2026-10-05 in #1617, #1627: a keyed agent create, spawn or clone retried with the same request returns its session even after a template or source edit, with no time limit; the host compares the request as the agent sent it instead of what it resolved to, and the helm's 30-day stored-resolution table and its machinery are deleted; a keyed `agent create` must name `--host`; the key-reuse refusal stops echoing the internal key
- [`get-site-cutover`](reports/get-site-cutover.report.md) landed 2026-10-05 in #1628, #1629, #1630, #1631: installs, helm payload downloads and the Mac app's updater switch to get.farhelm.io with signed checksums (installer and updater verified against a two-key ring, CI stops signing); PRs 1–4 of the cutover, README/docs switch excluded
