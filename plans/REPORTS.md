# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`omp-pane-guard-all-launches`](reports/omp-pane-guard-all-launches.report.md) landed 2026-10-10 in #1779: the OMP Resume ownership check refuses an unrecognised Bun or Node pane for every launch type, not only installed `omp`
- [`hook-report-watch`](reports/hook-report-watch.report.md) landed 2026-10-10 in #1776: the supervisor applies conversation reports as soon as they are written, via a file watch, with the timer as backstop
- [`sweep-on-timer`](reports/sweep-on-timer.report.md) landed 2026-10-10 in #1792: the supervisor sweeps on its 2 s timer only, and stops re-reading settled stopped sessions
- [`waiting-sound`](reports/waiting-sound.report.md) landed 2026-10-10 in #1768: sounds when a session waits, an approval arrives, or (opt-in) a turn finishes, each switchable per device in Settings
