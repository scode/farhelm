# Retention plan omits a live-retry downgrade limitation

Reviewed commit: 343faa698aae5bf2907aa1b84ec6bc9b5a08b667

## TLDR

The retention plan says older builds can open the converted database but does not disclose that those builds can refuse
recovery of a still-live checkout request. Its stated downgrade caveat covers deleted sessions only.

## Details

Suggested bucket: **other**, as an unresolved planning/disclosure boundary; potentially high if the refusal materially
blocks recovery. Confidence: **confirmed older-code predicate and planned conversion**, conditional on running that
older build on converted state. No deletion, termination or highest-priority consequence is established.

`plans/queue/create-reservation-retention.md:69–75` converts permanent reservations with live sessions to session
lifetime without changing the schema. The same revision's older managed-checkout reconciliation rejects every
non-permanent reservation (`crates/farhelm-supervisor/src/service/core.rs:7373–7381`). The plan itself identifies that
predicate as needing change at `138–151`. Old code can therefore decode/open the row but refuse semantic recovery. The
existing caveat says only that downgrade cannot restore deleted-session answers.

Possible cover: `SPEC.md:3493–3497` settles compatibility per feature and requires surfacing lost state or missing
downgrade paths. It does not clearly accept this live-request limitation; the named plan acceptance is narrower. No
exact Planned, known-bug, queue or ledger coverage was found. Proposed drop: none for the semantic residual. Unknown
encoding or schema-version rejection is separately refuted by the existing decoder and unchanged schema.

Verify the intended compatibility scope with the recorded feature decision before broadening implementation work; the
first correction may be a complete disclosure rather than preserving every historical recovery path.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g147/g024-older-supervisor-encoding`; synthesis
corrected the initial decoder-only exclusion.
