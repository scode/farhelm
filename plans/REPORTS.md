# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`compact-rows-test`](reports/compact-rows-test.report.md) landed 2026-10-15 in #2031: update the compact-rows and OMP composer browser tests to the permission wording since the guarded mode
- [`settings-lockout-scope`](reports/settings-lockout-scope.report.md) landed 2026-10-15 in #2032: keep other hosts' Settings usable during a one-click Update; block it only while a setup or uninstall confirmation may open
- [`goose-named-restart-test`](reports/goose-named-restart-test.report.md) landed 2026-10-15 in #2033: supervisor test that a named Goose restart clears the captured conversation in memory and a definitive failure restores it
- [`click-to-open-paths`](reports/click-to-open-paths.report.md) landed 2026-10-15 in #2034: Cmd-click a terminal file path in the desktop app to save and open it (.md, .txt, .html only), plus the redesigned file hover
- [`triage-power-loss-spec`](reports/triage-power-loss-spec.report.md) landed 2026-10-15 in #2039, #2043: Spec: downloads and new checkouts need not survive a power loss (two triage outcomes)
- [`triage-trusted-host-values-spec`](reports/triage-trusted-host-values-spec.report.md) landed 2026-10-15 in #2040, #2042: Spec: the user's own host names and paths are trusted in copied SSH help commands (two triage outcomes)
- [`triage-hand-edited-units-spec`](reports/triage-hand-edited-units-spec.report.md) landed 2026-10-15 in #2041, #2049: Spec: a hand-edited Farhelm service voids setup and uninstall guarantees (two triage outcomes)
- [`triage-highest-discards`](reports/triage-highest-discards.report.md) landed 2026-10-15 in #2050: Remove seven discarded review findings from the queue
- [`triage-conversation-tracking-spec`](reports/triage-conversation-tracking-spec.report.md) landed 2026-10-15 in #2038, #2044, #2045, #2046, #2047, #2048: Spec: Restart may resume the previous conversation in the cases the maintainer accepted (six triage outcomes)
- [`legacy-template-edits-gated`](reports/legacy-template-edits-gated.report.md) landed 2026-10-15 in #2051, #2052: Older templates: no silent command edits and no Undo that cannot work, only if trivially fixable
- [`confirmations-save-error`](reports/confirmations-save-error.report.md) landed 2026-10-15 in #2054: Settings shows an error when turning session confirmations back on fails to save
- [`post-clone-carriage-return-gated`](reports/post-clone-carriage-return-gated.report.md) landed 2026-10-15 in #2053: Post-clone command: no silent carriage-return changes on edit, only if trivially fixable
