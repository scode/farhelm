# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`drag-copy-notice`](reports/drag-copy-notice.report.md) landed 2026-10-09 in #1731: The "drag copied nothing" notice appears just above the pointer, stays up to 30 seconds with a dismiss button, and shows on every such drag.
- [`preview-lock-identity`](reports/preview-lock-identity.report.md) landed 2026-10-09 in #1732: The docs preview script leaves alone a process that started after the stale lock naming it was written.
- [`transcript-reads-on-need`](reports/transcript-reads-on-need.report.md) landed 2026-10-09 in #1734, #1736, #1738: The supervisor stops re-reading Codex and Grok transcripts on every pass (a later Codex hook confirms a `/clear`, Restart catches a deleted file) and drops the other per-pass overhead: empty report-folder renames, uncached statements, no-op notification writes.
