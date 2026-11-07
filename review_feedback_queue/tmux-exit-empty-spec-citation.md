# Point the exit-empty rationale to its actual specification

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The new configuration explanation directs maintainers to a specification section in the wrong document. Its description
of when the private server ends is accurate; the error is its source citation.

## Details

Source coordinates from the original records: `crates/farhelm-supervisor/src/tmux.rs:2376-2378`. References below use
these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

The exit-empty bullet at2376-2378 cites SPEC.md, Terminal substrate for the policy that no tmux runs after the last old
terminals disappear. Searching pinned SPEC.md for terminal/substrate/tmux headings, Terminal substrate and exit-empty
finds no such section; its relevant product heading is Terminal experience at2371. The actual implementation contract is
SPEC_impl.md, Terminal substrate: private tmux server at2807, whose2818-2831 specifies exit-empty, lazy old creation,
adopted inventory and stopping tmux traffic after absence. Change this reference to SPEC_impl.md and its existing
heading so a maintainer can find the cited policy. The introduced version at2502-2504 has the same wrong filename; base
config_body at2540-2551 uses exit-empty off and does not have this new bullet. Thus the wrong citation is introduced in
range and survives tip.

This is distinct from retained q2's nonexistent rustdoc method target, g033/f8's incorrect negative query-gate promise
and g032/f5's stale error-result promise. No runtime invariant, misleading behavioral guarantee, rustdoc-build failure
or user-work-loss consequence is attributed to this citation. Suggested bucket other. Confidence confirmed pinned
source/spec comparison and introduction ancestry. proposed_drop false, following parent's explicit disposition that
introduced actionable source-documentation navigation errors have no generic value exclusion. possible_cover none among
latest relevant named peer records read. No runtime experiment or checkout edit.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g031/r4`. The retained
append-only histories preserve all earlier claims and corrections. Reviewed source references name their own revisions;
the inclusive reviewed tip is the hash above. This review used source inspection only, with no product runtime
experiments or live-install changes.
