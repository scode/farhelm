# Correct the refusal helper's internal link

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

An internal rustdoc link names a method the driver does not have. The refusal behavior and surrounding explanation
remain accurate; no product consequence is established.

## Details

Source coordinates from the original records: `crates/farhelm-supervisor/src/tmux.rs:2573-2574`. References below use
these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

The doc above TmuxDriver::unusable_refusal says the error is also given by supervisor refusals and links
Self::unusable_reason (2573-2574). Full-file definition/reference search finds no TmuxDriver method with that name. The
adjacent actual entry point is unusable_refusal_if_any at2587-2590, which reads self.presence.unusable_reason then
constructs this error. unusable_reason belongs to the presence model, not this owning impl. At introduction the same
nonexistent Self link already occurs. Correcting the link to Self::unusable_refusal_if_any would restore navigation
without changing the stated refusal contract. This is not g033/f8's false-only-if-absent behavior promise or g032/f5's
stale error-result contract. SPEC_impl.md Terminal substrate: private tmux server:2831-2840 agrees with the refusal
explanation; no contradictory lifecycle instruction was found here. No rustdoc build or runtime test was run; the exact
unresolved method target is a source fact, not a measured build failure.

Suggested bucket other. Confidence confirmed source target mismatch. proposed_drop true: only internal navigation
precision is demonstrated; the adjacent correctly named accessor, warning wording, and behavioral explanation remain
correct, with no new invariant/failure-mode misinformation or product effect. This is a proposed recording-value
exclusion for the commissioner, not a filter lookup, source refutation or spec acceptance of a different behavior. Could
be highest: no security or user-work-loss route from this link. possible_cover none; g036/c4 is an analogous but
separate stale test-comment identifier, not duplicate cover. No new verified runtime or investigated-unverified
behavioral finding from q2.

Disposition acknowledgement before renewed discovery. Parent explicitly retains q2 as other: the nonexistent rustdoc
target was introduced in range and is an actionable source-documentation error. Neither recording rules nor FILTER has a
generic navigation-only/value exclusion. My earlier proposed navigation-only exclusion is withdrawn; historical proposal
and its exact basis remain in prior append-only event. No FILTER or other forbidden coverage source was searched by this
reviewer. Confidence remains confirmed source target mismatch, no measured rustdoc build failure or misleading
behavioral guarantee claimed. Full reviewed commit407a49435278844248d573083fdea6e23fd96c02, introduction
f972458cc78eea039ab185354d13cf8bd395c982 and fix direction remain in q2 history. Suggested bucket other; proposed_drop
false; possible_cover none. This is disposition synthesis, not new discovery. q3 and named incoming g009/d17 remain
additional evidence for known e2, not independent roots; all original conditional confidence, finite-traffic
consequences and unmeasured-incidence caveats preserved.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g031/q2`. The retained
append-only histories preserve all earlier claims and corrections. Reviewed source references name their own revisions;
the inclusive reviewed tip is the hash above. This review used source inspection only, with no product runtime
experiments or live-install changes.
