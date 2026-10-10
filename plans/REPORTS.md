# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`browser-terminal-test-oracles`](reports/browser-terminal-test-oracles.report.md) landed 2026-10-10 in #1819: terminal browser tests for empty frames, reconnect deadlines, heartbeats, link drags, delete holds, phantom tabs and mouse reports, and the client-log JS test, assert what they claim
- [`ui-correctness-fixes`](reports/ui-correctness-fixes.report.md) landed 2026-10-10 in #1832: launcher, terminal, sidebar and dialog fixes: Grok trust, dot template names, Unicode template match, IME save, A+/A- focus, OSC 8 drag, listing fence, read-mark retry, update-popup scroll, stale setup errors, escaped refusals and titles
