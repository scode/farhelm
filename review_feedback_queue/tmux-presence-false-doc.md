# Document that a suppressed query is not proof a session is absent

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The new internal API documentation says a false query-gate answer proves an old session does not exist. It also returns
false when a server is unusable and its agents may still run. Future maintainers are given an incorrect contract; no
additional shipped process-loss route is attributed to the prose.

## Details

Source coordinates from the original records: `crates/farhelm-supervisor/src/tmux/presence.rs:38-41,107-113,153-159`.
References below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

The docstring at presence.rs:153-154 says 'false only when this process knows it does not (no server, or a server
without it)'. Its actual match at 155-159 returns false for Knowledge::Unusable as well as Absent. This contradicts
knows_absent:134-143, which explicitly describes unusable sessions as unknown and returns false, and
Knowledge::Unusable:76-83, which says agents may still run and destructive assumptions must be refused. The query gate
means do not query, not the stronger absence fact the new text promises.

Discovery checked the public wrapper too: tmux.rs:2627-2638 explains may_hold_session in terms of missing names/no
server and delegates to may_hold; although it lacks the inner method's explicit word 'only', it likewise omits the
unusable case. The wrapper's neighboring knows_session_absent contract at 2593-2599 correctly warns that
!may_hold_session is not conclusive absence for an unusable server. Update the assigned inner contract and qualify the
wrapper's examples, pointing lifecycle callers to the conclusive-absence method when they need it. Leave the intentional
Unusable=false query gating unchanged.

SPEC_impl.md, Terminal substrate: private tmux server, 2826-2840 requires queries to be suppressed while unusable even
though agents may live there, with separate destructive refusals. This supports the implementation and contradicts the
claimed false-only-if-absent invariant. This is internal behavior-contract guidance, not an identifier spelling cleanup
or a new version of the already known Delete-ordering bug.

Suggested bucket: other. Confidence: confirmed source contradiction and intended specification distinction; no
runtime/user-work-loss consequence from this prose alone is claimed. proposed_drop: false. possible_cover: g032/f5
concerns a different outdated error contract in is_definitively_empty; g031/f4/g025's actual late-refusal bug is
separately known. Neither is this newly introduced negative-result promise in the assigned presence API. Follow-up
wrapper check is complete; no unresolved premise for this documentation finding. No runtime experiments.

Reviewed tip407a49435278844248d573083fdea6e23fd96c02, assigned f972458 presence.rs. Fresh documentation scan adds a
precise corroborating location to existing f8, not a separate root: may_run field documentation says 'anything but
Knowledge::Absent' (38-39), but publish107-108 suppresses both Absent and Unusable. Like may_hold's
false-only-when-known-absent promise, this omits the intentionally suppressed unknown-live Unusable state. Update the
watch field contract too when fixing f8; retain intended false watch behavior. SPEC_impl2831-2840 requires suppression
for unusable even when agents live. Existing f8 bucket other and confirmed source-documentation contradiction remain; no
additional shipped work-loss consequence or runtime claim, no bucket change. proposed_drop false; possible_cover
existing own f8, same omitted-Unusable negative-result distinction, not g032/f5's separate stale has_session-error
contract or g031/r4's wrong specification filename. This is newly located support for a known finding, not a genuinely
new independently counted finding. All preceding f8 details remain intact.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g033/f8`. The retained
append-only histories preserve all earlier claims and corrections. Reviewed source references name their own revisions;
the inclusive reviewed tip is the hash above. This review used source inspection only, with no product runtime
experiments or live-install changes.
