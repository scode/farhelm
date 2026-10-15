# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 12d6b1] `click-to-open-paths.md` — Cmd-click a terminal file path in the desktop app to save and open it (.md, .txt, .html only), plus the redesigned file hover
- [in-flight 7018db] `triage-conversation-tracking-spec.md` — Spec: Restart may resume the previous conversation in the cases the maintainer accepted (six triage outcomes)
- [in-flight bd3e8f] `triage-power-loss-spec.md` — Spec: downloads and new checkouts need not survive a power loss (two triage outcomes)
- [pending] `triage-trusted-host-values-spec.md` — Spec: the user's own host names and paths are trusted in copied SSH help commands (two triage outcomes)
- [pending] `triage-hand-edited-units-spec.md` — Spec: a hand-edited Farhelm service voids setup and uninstall guarantees (two triage outcomes)
- [pending] `triage-highest-discards.md` — Remove seven discarded review findings from the queue
- [pending] `confirmations-save-error.md` — Settings shows an error when turning session confirmations back on fails to save
- [pending] `legacy-template-edits-gated.md` — Older templates: no silent command edits and no Undo that cannot work, only if trivially fixable
- [pending] `post-clone-carriage-return-gated.md` — Post-clone command: no silent carriage-return changes on edit, only if trivially fixable
- [pending] `guarded-default-and-escalation-banner.md` — Guarded is the default for every agent that supports it (OMP changes), and a banner when an agent switch escalates the permission mode
