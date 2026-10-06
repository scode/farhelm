# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.
- [in-flight 2d44a9] `templates-dialog-overhaul.md` — Redesign the Templates panel (list beside editor, only the fields a template sets, choices that follow the agent type, undo for delete) and have new templates carry their launch kind.
- [complete] `clone-into-fresh-checkout.md` — Clone on a session in a GitHub checkout opens the launcher on a fresh `gh:` checkout of the same repository, named after the session plus `-clone` (first free `-clone-N` when taken); choosing a folder gives today's Clone; Replace with, Replace and `farhelm agent clone` unchanged; UI-only
