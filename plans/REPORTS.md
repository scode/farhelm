# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`hover-help`](reports/hover-help.report.md) landed 2026-10-04 in #1589, #1590: every icon and clickable control gets hover text in a fast, themed tooltip of Farhelm's own (300 ms, above the control), replacing the browser's slow native tooltips, with a browser test that fails on any control without one
