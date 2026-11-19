# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `exited-session-no-terminal.md` — An exited session whose terminal is gone shows Restart and Replace instead of reconnecting forever (protocol bump)
- [pending] `gui-ux-wording.md` — Comment, spec and docs corrections from the 2026-11-18 GUI UX triage, two spec acceptances and the user's-own-configuration principle
- [complete] `gui-ux-desktop-fixes.md` — Header question anchors, Close all after navigation, selected-template contrast, host text in tooltips and refusals, wrong action names, approval keyboard tab stops
- [in-flight ce1220] `gui-ux-backend-fixes.md` — Bounded shell-tab requests, Tensorlake suspension attribution, late approval answers, tunnels for retired sessions, the Mac update wait, endpoint @ paths, braced command display
- [pending] `antigravity-support.md` — Support Google Antigravity (agy): structured launches, sidebar mark, user-installed hook conversation tracking, and docs (protocol bump)
