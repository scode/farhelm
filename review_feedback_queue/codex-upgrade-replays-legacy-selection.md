# An upgrade can restore a discarded Codex conversation

Reviewed commit: d2f4508e01366c15ade8a84dff988a6400ef16ba

## TLDR

A Codex session that keeps running while Farhelm is closed can have its newer conversation replaced by an older saved
report after Farhelm is upgraded. Restart can then resume the conversation the user had discarded.

## Details

Suggested bucket: `highest`. Confidence: likely; the mixed-version, offline upgrade sequence was not reproduced with the
desktop app and installer. Source: adversarial capture reviewer, with parent inspection of the report drain and
admission code. Proposed drop: none.

At v0.25.0, `HookReport::slot` sends every Codex report to `latest.json`. Commit
`f7f0b7d98b98cf31802e7707bad20cce4e7b9443` moves Codex SessionStart reports to `selection.json` and Stop reports to
`enrichment.json` (`crates/farhelm-supervisor/src/agent_kind/mod.rs:2014–2036`), without changing report format version
1 (`crates/farhelm-supervisor/src/hook_report.rs:65`). Both generations of report files remain readable.

The drain takes all available slots, then applies Selection, Enrichment, Latest in that order (`hook_report.rs:107`,
`service/report_files.rs:454–508`). A pending old **SessionStart** report selecting conversation A in Latest is
therefore applied after a newer selecting report for conversation B. Codex admission permits a different runtime or
exact transcript to replace the current binding and has no chronology fence (`service/core/vendor/codex.rs:166–241`).
A's still-valid transcript and the unchanged surviving pane satisfy its ownership proofs even though the user has moved
on to B.

To verify, close Farhelm while leaving a Codex session running, let the old hook publish A, upgrade the installed binary
while the supervisor remains absent, and have that same Codex process select B through a hook running the new binary.
Reopen Farhelm with both files pending. Verify the stored conversation and Restart's selected thread after startup
drains them. Use a selecting event for A; an old Stop report alone does not establish this rollback. A lower-level
regression can supply compatible old/new reports with the same live-pane ancestry and distinct valid transcripts.

Possible cover: SPEC_impl.md's "What running sessions hold across versions" explicitly says surviving hooks run the
newly installed binary (`3923–3933`). Its accepted limitation for older Codex launches lacking Stop concerns readiness,
not replacement by an older conversation. `TRIAGE_OUTCOMES.md`,
`restart-resume-older-conversation-report-drain-fails.md`, accepts specific crash or coincident-fault outcomes; this
sequence needs neither. TODO.md's nested Grok slot-erasure item concerns another vendor and mechanism. No exact existing
queue or accepted decision was found.

The compatibility boundary needs to prevent a legacy selecting slot from overriding a newer Codex selection. Preserve
legitimate legacy-only reports and the independent selection/enrichment outage behavior.
