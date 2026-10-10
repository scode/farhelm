# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`ssh-config-atomic`](reports/ssh-config-atomic.report.md) landed 2026-10-10 in #1805: the CentOS test edits the user's ssh config by atomic rename, and a failed step never overwrites it
- [`docs-and-capture-fixes`](reports/docs-and-capture-fixes.report.md) landed 2026-10-10 in #1809: the uninstall guide stops promising Mac file checks, light-mode docs headings are readable, the desktop build recipe exports its target, and README and video captures refuse stale builds
