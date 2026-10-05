# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.
- [complete] `triage-tooling-output-ownership.md` — Maintainer video recorder and screenshot publisher delete or drop only what they made.
- [in-flight 6201b7] `triage-dialog-cancel-races.md` — Cancelled YOLO questions and a closing feedback dialog stop acting on queued answers (complexity-gated).
- [in-flight 49d56a] `triage-install-uninstall-gaps.md` — Escape remote linger errors, exact uninstall identity check, installer refuses symlinked app folders.
- [in-flight 27b6e7] `triage-template-gaps.md` — Templates refuse to overwrite on an unloaded list and refuse a replaced machine at dispatch.
