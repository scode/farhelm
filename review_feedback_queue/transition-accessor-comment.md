# Adjacent test documentation retains the old accessor name

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

A maintainer following the test's explanation encounters an accessor name that no longer exists. No product behavior or
lifecycle guarantee is shown to be wrong.

## Details

Source coordinates from the original records: `crates/farhelm-supervisor/src/store.rs:9968`. References below use these
paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

The text at store.rs:9968 says `Transition::pane` while the method is now `Transition::address` (:532). This text itself
is UNCHANGED ADJACENT CONTEXT: before b7f5203 the identical phrase appears at store.rs:9442; the old accessor is at
:528. The assigned diff changes the test's fields/assertion from pane to terminal_address/address, not the surrounding
transactional explanation. That explanation still says the terminal address and outcome must be written in one
statement, and :9972-10005 still asserts both values. This is an accessor-name reference made stale by the rename, not
an introduced or changed behavioral, lifecycle, async, failure, or ownership contract; it is plain code formatting, not
an executable example or a rustdoc link. A possible correction is `Transition::address`. Suggested bucket: other.
Confidence: confirmed source fact, including pre-change location. proposed_drop: the only demonstrated defect is
navigational precision of an unchanged adjacent internal test comment; its specification of the atomic-write contract
and its oracle remain accurate, and there is no established misleading lifecycle instruction or user-visible
consequence. These specific facts are the proposed recording-value/scope basis, NOT a claim of source refutation, spec
acceptance, or a filter match. The commissioner decides whether that basis warrants exclusion. possible_cover: none.
Cannot be highest on this established route. Earlier revisions, including their less precise low-impact labels and the
initially wrong candidate classification, remain preserved in this append-only log.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g036/c4`. The retained
append-only histories preserve all earlier claims and corrections. Reviewed source references name their own revisions;
the inclusive reviewed tip is the hash above. This review used source inspection only, with no product runtime
experiments or live-install changes.
