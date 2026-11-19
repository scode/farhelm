# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `gui-ux-wording.md` — Comment, spec and docs corrections from the 2026-11-18 GUI UX triage, two spec acceptances and the user's-own-configuration principle
- [pending] `antigravity-support.md` — Support Google Antigravity (agy): structured launches, sidebar mark, user-installed hook conversation tracking, and docs (protocol bump)
