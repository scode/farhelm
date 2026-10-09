# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`repo-clone-cache`](reports/repo-clone-cache.report.md) landed 2026-10-09 in #1720: Fresh GitHub checkouts go through a per-host repository cache in the supervisor's state directory, so only new objects cross the network; unused caches are removed after 30 days.
