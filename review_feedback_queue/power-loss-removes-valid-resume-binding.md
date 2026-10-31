# Power loss can remove an already valid Resume binding

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A power loss can remove or replace an already accepted Resume offer after its report has been acknowledged and deleted.

## Details

Suggested bucket: **highest**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The accepted capture commits at WAL/NORMAL, then successful reporting is acknowledged and its file unlinked. Reboot can
remove both the producer/pane anchor and the unsynced captured binding, leaving no ordinary source from which to restore
the previously valid offer. This concerns an admitted offer, separately from the first report never being accepted. No
vendor conversation transcript deletion is alleged. SPEC_impl.md:2991–3004's recent captured-state-loss allowance
conflicts with SPEC.md:2474–2485's specific recovery promise and the differently triggered exceptions at
2442–2449,2499–2500. Independent reviewers retain that contract ambiguity; no physical loss was executed.

Source locations:
`crates/farhelm-supervisor/src/db.rs:113-115; crates/farhelm-supervisor/src/store.rs:5065-5078; crates/farhelm-supervisor/src/service/report_files.rs:248-251,522-525`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g178/f4`, `a017/g178-f4`, `a047/audit-g178-f4`.
These are original reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and
independent verdicts remain in the private review evidence. No fix or triage decision was executed.
