# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`untrusted-text-escaping`](reports/untrusted-text-escaping.report.md) landed 2026-10-10 in #1797: status badges and the sidebar's folder line show host text with hidden characters made visible, and the helm logs malformed or refused supervisor messages escaped
- [`git-env-isolation`](reports/git-env-isolation.report.md) landed 2026-10-10 in #1798: an inherited GIT_DIR no longer steers checkout preparation or the discovery test fixtures into another repository
- [`os-readback-fixes`](reports/os-readback-fixes.report.md) landed 2026-10-10 in #1796: the service-file reader refuses escapes and section spellings systemd reads differently, and macOS reads large hook argument blocks whole
- [`harness-tooling-fixes`](reports/harness-tooling-fixes.report.md) landed 2026-10-10 in #1799: quoted fixture paths, a per-run spawn-test workspace, deflake stop checks process start time, full hostname scrubbing, and release advice that never reuses a tag
- [`ssh-config-atomic`](reports/ssh-config-atomic.report.md) landed 2026-10-10 in #1805: the CentOS test edits the user's ssh config by atomic rename, and a failed step never overwrites it
- [`docs-and-capture-fixes`](reports/docs-and-capture-fixes.report.md) landed 2026-10-10 in #1809: the uninstall guide stops promising Mac file checks, light-mode docs headings are readable, the desktop build recipe exports its target, and README and video captures refuse stale builds
