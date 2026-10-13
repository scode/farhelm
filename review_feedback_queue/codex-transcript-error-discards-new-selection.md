# A transcript read error discards a new Codex selection

Reviewed commit: d2f4508e01366c15ade8a84dff988a6400ef16ba

## TLDR

A filesystem error while recording Codex's new conversation can discard its report permanently while leaving the old
conversation available for Restart. A later turn-completion report cannot select the new conversation.

## Details

Suggested bucket: `highest`, because Restart can target a discarded conversation. Confidence: confirmed static control
flow; no filesystem-failure or live-Codex reproduction. Source: adversarial capture reviewer, with an independent check
of the baseline and proposed coverage. This is a pre-existing issue discovered in the changed capture path, not an
introduced-in-range regression.

A valid SessionStart clear selecting B can hit an OS error when its exact transcript is read.
`crates/farhelm-supervisor/src/service/core/vendor/codex.rs:185–190` maps `locator.verify()` errors to InvalidRequest.
The report drain retries Internal errors or a changed generation, but settles and deletes this refusal
(`service/report_files.rs:246–265,520–524`). The previously admitted A remains. Stop for B is enrichment only and cannot
select a different runtime (`service/core/vendor/codex.rs:144–149`), so restored filesystem access alone does not
correct that lost selection. Restart can still use A.

Begin with A admitted, supply a valid B selecting report with a controlled transcript-open I/O error, and establish
settlement plus the unchanged A binding. Restore reads and submit Stop B; verify that it still does not select B.
Distinguish transient OS failures from malformed vendor evidence so a retryable selecting report can remain pending.

Proposed drop: the original reviewer proposed exclusion as pre-existing, or coverage under the healthy-filesystem
assumption. The independent reviewer confirmed that v0.25.0 already converted the error to InvalidRequest and settled
the report, but did not agree that the specification explicitly accepts the wrong-conversation consequence. The finding
is retained with that scope caveat rather than attributed to the newer Stop path.

Possible cover: SPEC.md:2178–2188 accepts filesystem-induced failures or stalled progress on an affected host. It does
not explicitly discuss preserving an older Resume offer after permanent selection loss. TRIAGE_OUTCOMES.md's
`restart-resume-older-conversation-report-drain-fails.md` concerns retained, retryable reports under specific crash or
coincident-fault conditions, rather than this permanently discarded report. Whether the filesystem acceptance extends to
this consequence needs triage; no exact existing queue item was found.
