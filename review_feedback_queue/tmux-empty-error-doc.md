# Pane-listing documentation describes the old absence contract

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

No current user-visible failure is established. The code's explanation of how stopped old terminals are detected
contradicts its new behavior, making future lifecycle changes easier to get wrong.

## Details

Source coordinates from the original records:
`crates/farhelm-supervisor/src/tmux.rs:4774-4786;crates/farhelm-supervisor/src/tmux/presence.rs:153-159`. References
below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

The assigned change makes has_session return false for an absent server (tmux.rs:3454-3475), as required by
SPEC_impl.md, Terminal substrate: private tmux server, 2818-2840. The nearby pane_states documentation still says
has_session 'DELIBERATELY propagates it as an error' and claims its caller cannot safely infer a persisted row's
terminal is gone (4774-4786). The stale explanation teaches the opposite safety distinction from the implementation
introduced by f972458; the current distinction is absent versus unusable, documented at 2555-2565. Update or remove that
paragraph to match the new contract.

Suggested bucket: other. Confidence: confirmed. proposed_drop: false (documentation of a lifecycle safety contract is
materially misleading). possible_cover: none. Could be highest: no; no shipped runtime harm follows from this prose
alone.

Synthesis revision; reviewed commit 407a49435278844248d573083fdea6e23fd96c02. Re-read surrounding module documentation
4765-4825, not only the stale sentence. This is material contract guidance rather than an old name: 4781-4786 says
has_session 'DELIBERATELY propagates it as an error' because a persisted-row caller 'cannot safely guess' absence;
current has_session at 3454-3475 actually returns Ok(false) for NoServer and SocketMissing. The next paragraphs say
listings tolerate positive absence but 'Any OTHER pane_states failure ... still propagates as a real Err' and
unclassified failures are 'genuinely UNKNOWN' (4819-4823). The obsolete carve-out therefore asserts a nonexistent safety
distinction in the API used to decide teardown/retry behavior. The current distinction is Absent versus Unusable/unknown
(2555-2565), not listing versus session probe for the same NoServer fact. Retain the finding, suggested bucket other,
confidence confirmed, proposed_drop false, possible_cover none. No user-work loss or runtime failure is attributed to
this prose itself. Fix can be confined to replacing/removing that paragraph and preserving the valid
unknown-versus-absence explanation around it.

Read latest g033:f8 in full before fresh discovery. It is already known and distinct from f5: f8's newly added may_hold
doc promises false only with conclusive absence, while the match also returns false for Unusable. f5's separate
pane_states documentation falsely says has_session deliberately propagates positive NoServer as an error; its actual
implementation returns false. Both are other/source-confirmed documentation contracts, with no additional runtime or
user-work-loss consequence attributed to prose. Keep IDs separate; do not treat them as mutual cover or count f8 as
newly discovered here. f5 remains open, proposed_drop false, possible_cover none for its exact obsolete error-result
contract. Incoming g031:d1 is another separate runtime finding (gone during version probe becomes Unusable), not either
documentation defect.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g032/f5`. The retained
append-only histories preserve all earlier claims and corrections. Reviewed source references name their own revisions;
the inclusive reviewed tip is the hash above. This review used source inspection only, with no product runtime
experiments or live-install changes.
