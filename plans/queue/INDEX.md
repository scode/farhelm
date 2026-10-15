# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 1b33bf] `confirmations-save-error.md` — Settings shows an error when turning session confirmations back on fails to save
- [landing 55f6b8] `legacy-template-edits-gated.md` — Older templates: no silent command edits and no Undo that cannot work, only if trivially fixable
- [complete] `post-clone-carriage-return-gated.md` — Post-clone command: no silent carriage-return changes on edit, only if trivially fixable
- [in-flight 215ee8] `guarded-default-and-escalation-banner.md` — Guarded is the default for every agent that supports it (OMP changes), and a banner when an agent switch escalates the permission mode
