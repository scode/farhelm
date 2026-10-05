# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`hook-report-files`](reports/hook-report-files.report.md) landed 2026-10-05 in #1585, #1594, #1595: conversation hooks write their latest report into fixed slot files in a per-session drop directory that the supervisor applies on its reconciliation pass, replacing the socket round trip and its retries and timeouts, so tracking survives the supervisor being down (the Mac app closed); attribution runs on the hook's recorded process chain; the spec states that agents' `farhelm` commands fail while the supervisor is down
