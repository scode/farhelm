# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`templates-dialog-close`](reports/templates-dialog-close.report.md) landed 2026-11-21 in #4345, #4346, #4347: let the Templates dialog close: no save question for an untouched template, and a leave question shown over the dialog (templates and the Settings recipe editor)
- [`agent-add-host-takeover-hint`](reports/agent-add-host-takeover-hint.report.md) landed 2026-11-21 in #4354, #4357: point users to Add host (instead of a non-existent Take over row in Hosts) when an agent asks to add a host managed by another Farhelm
