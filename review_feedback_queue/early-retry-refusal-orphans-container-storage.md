# Early retry refusal can orphan container storage

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A refused pending-create retry can discard its session handle while an earlier container's storage remains.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Independent synthesis establishes the compatible existing-folder membership, pre-confirmation allocation, reboot with no
queried launch evidence, changed-template retry refusal and another same-checkout member that permits row deletion. The
early refusal precedes the later retained-wrapper/container cleanup. Complete initialization or user-written files are
unnecessary for this storage-orphan consequence. This record supersedes earlier claims that the ordinary producer
remained missing. It is separate from deleting initialized writable-layer files and from final-confirmation rollback.

Source locations:
`crates/farhelm-supervisor/src/service/core.rs:8914-8930,9596-9635; crates/farhelm-supervisor/src/store.rs:5657-5690`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Record repair/synthesis following parent reconciliation of independent a032 synthesis2, retaining a003/d1/a032
provenance. Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d. The conditional source history is complete; prior
g115 wording that complete affected-history composition remained unproven is superseded, not erased. Exact
prerequisites: an ordinary existing-folder container launch has checkout membership; a generation-zero predecessor
container is allocated before the pane is durably confirmed; reboot occurs before first recovery while the session has
no recorded pane/conversation and ordinary agent-scope/sentinel/tmux launch evidence is absent; the keyed agent request
is retried after its referenced template changes, preserving the raw request digest but changing resolved launch inputs;
another member of the SAME nonretired checkout permits borrower-row rollback. Independent a032 checked membership
store.rs:2986-2990, allocation-before-confirmation container.rs:567-640/core.rs:10842-10869, reboot/evidence
core.rs:6475-6497,6606-6629,6771-6843, early mismatch/refusal core.rs:8914-8930,9596-9635, and rollback
store.rs:5657-5690. Parent directly supplied that synthesis; g115 does not claim new direct reads of every a032 range.
Prior g115 direct reads independently establish the changed-template keyed-request producer, refusal before
retained-origin wrapping, no container cleanup on row/map deletion, generation-zero preservation, pending reservation
survival, no Launching-outcome rollback predicate and same-checkout last-reference guard. Consequence: Farhelm deletes
the ordinary ownership handle without removing the predecessor container, orphaning persistent container storage.
Complete initialization or user work inside the predecessor is unnecessary for this OTHER cleanup/ownership consequence.
Confidence: source-confirmed conditional route; occurrence unmeasured. No empirical composition gap, universal absence
proof or runtime reproduction is required. No actual source contradiction remains identified. Suggested bucket other.
proposed_drop none. possible_cover: SPEC.md:1498-1501,1515-1527 may accept exact earlier preconfirmation-crash
process/ownership loss only where consequence scope matches; no blanket storage-orphan acceptance and no waiver of
separate later writable-layer deletion reported by g113. f1 healthy-host confirmation-error producer remains an
investigated uncertainty, and f2 namespace HIGH/both-images/scoped-Delete limits remain unchanged.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g115/f4`, `a003/a003/d1`, `a004/x8`, `a005/f7`,
`a032/a003/d1`. These are original reviewer identities for this concern, not additional bugs. Full revisions,
withdrawals and independent verdicts remain in the private review evidence. No fix or triage decision was executed.
