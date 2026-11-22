# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`triage-supervisor`](reports/triage-supervisor.report.md) landed 2026-11-22 in #4520, #4527, #4529, #4530, #4534, #4538, #4543, #4546, #4552: execute the 2026-11-21 triage outcomes for the supervisor (session lifecycle, shell sessions, cleanup), in grouped PRs
- [`maximized-window-no-zoom-animation`](reports/maximized-window-no-zoom-animation.report.md) landed 2026-11-22 in #4547, #4557: open a Mac window that was maximized at quit already zoomed, without the grow animation, keeping the ordinary frame a later zoom returns to
- [`deflake-historical-base-prereqs`](reports/deflake-historical-base-prereqs.report.md) landed 2026-11-22 in #4550: refuse deflake sweeps of bases older than the current battery list, with an explanation, before anything starts
