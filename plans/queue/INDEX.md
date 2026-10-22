# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `popover-anchor-close.md` — Route closing anchored popovers on row movement through one owner, and close the three movements left uncovered on purpose (structural refactor; aligned fixes in their own PRs).
- [complete] `triage-specs-oct21.md` — Write down two accepted behaviors (folders someone else put in the prepared-checkout container, failed-launch excerpts quoting the preceding run) and close two discarded findings (spec and bookkeeping only).
- [in-flight eac6f2] `gui-saves-last-write-wins.md` — Accept last-write-wins for GUI template and number-key saves in SPEC.md, and stop a failed number-key save from later writing its stale map over newer agent changes.
- [in-flight 1ef316] `cheap-ui-hardening-oct21.md` — Three cheap GUI fixes, each discarded instead if it is not cheap: opaque desktop menu markers, unambiguous identities in the changed-host warning, and a numeric-version guard without slow backtracking.
- [complete] `start-remedy-quoting.md` — Offer the copyable supervisor-start command only when it survives the diagnostic's escaping unchanged.
- [in-flight c3f13a] `uninstall-mixed-spelling.md` — Host uninstall refuses when a home alias hides that kept data lies inside the folder it removes.
- [in-flight 9c13aa] `refused-retry-rollback-fence.md` — A refused create retry no longer deletes a session that was restarted meanwhile.
- [pending] `bind-gui-answers.md` — Bind every GUI answer to what the user saw: one shared prompt mechanism with opening tokens, click-time preconditions, declarative link covers and retired held input, plus the six GUI outcomes folded in at the 2026-10-21 triage (after `popover-anchor-close.md`)
