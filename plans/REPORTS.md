# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`get-site-cutover`](reports/get-site-cutover.report.md) landed 2026-10-05 in #1628, #1629, #1630, #1631: installs, helm payload downloads and the Mac app's updater switch to get.farhelm.io with signed checksums (installer and updater verified against a two-key ring, CI stops signing); PRs 1–4 of the cutover, README/docs switch excluded
