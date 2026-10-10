# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`omp-pane-guard-all-launches`](reports/omp-pane-guard-all-launches.report.md) landed 2026-10-10 in #1779: the OMP Resume ownership check refuses an unrecognised Bun or Node pane for every launch type, not only installed `omp`
- [`hook-report-watch`](reports/hook-report-watch.report.md) landed 2026-10-10 in #1776: the supervisor applies conversation reports as soon as they are written, via a file watch, with the timer as backstop
- [`sweep-on-timer`](reports/sweep-on-timer.report.md) landed 2026-10-10 in #1792: the supervisor sweeps on its 2 s timer only, and stops re-reading settled stopped sessions
- [`waiting-sound`](reports/waiting-sound.report.md) landed 2026-10-10 in #1768: sounds when a session waits, an approval arrives, or (opt-in) a turn finishes, each switchable per device in Settings
- [`terminal-file-download`](reports/terminal-file-download.report.md) landed 2026-10-10 in #1769, #1771, #1773: path-shaped text in a terminal is checked on hover and downloads from the session's host on click (100 MB limit; desktop saves to Downloads)
- [`untrusted-text-escaping`](reports/untrusted-text-escaping.report.md) landed 2026-10-10 in #1797: status badges and the sidebar's folder line show host text with hidden characters made visible, and the helm logs malformed or refused supervisor messages escaped
- [`git-env-isolation`](reports/git-env-isolation.report.md) landed 2026-10-10 in #1798: an inherited GIT_DIR no longer steers checkout preparation or the discovery test fixtures into another repository
- [`os-readback-fixes`](reports/os-readback-fixes.report.md) landed 2026-10-10 in #1796: the service-file reader refuses escapes and section spellings systemd reads differently, and macOS reads large hook argument blocks whole
- [`harness-tooling-fixes`](reports/harness-tooling-fixes.report.md) landed 2026-10-10 in #1799: quoted fixture paths, a per-run spawn-test workspace, deflake stop checks process start time, full hostname scrubbing, and release advice that never reuses a tag
