# Sandbox lifecycle questions omit opening and claim ownership

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

Sandbox lifecycle questions lack the required opening-token and operation-claim ownership. Same-dialog Cancel and Submit
before redraw is product-accepted when its destructive guards hold; a stronger obsolete-opening loss route remains
unverified.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `952183b99721b44fd6fef27b5ca3b38499165b7f`. Original source location:
`crates/farhelm-ui/src/sandbox_sessions.rs:187-211,248-266`. Locations below use that revision unless another is named;
the assessment was checked through the reviewed tip.

Controls stores an Option<Operation> and a separate held operation guard instead of using ConfirmSlot
(sandbox_sessions.rs:187-213). Cancel/Escape clear them without an opening token (:248-263); Confirm unconditionally
executes (:264-266). A later Cancel can also release the local guard after execution has begun. SPEC_impl.md:1042-1071
requires rendered opening identity and task-owned claim transfer; ops.rs:317-349 explains the first-answer-wins
requirement.

The source-supported same-dialog Cancel-then-Confirm Stop route remains in the history, but SPEC.md:4856-4859 explicitly
permits that burst when the matching broad warning and destructive preconditions still hold. Fresh independent
consultations agreed narrowly that this consequence alone is product-accepted and the retained implementation defect is
other. The earlier orphan-Delete ledger precedent also includes a separate cross-opening route and does not withdraw
that product text.

Stronger obsolete-opening substitution remains unverified after investigation: matching Dioxus source resolves current
mounted listeners, and no complete shipped cross-opening/element-ID sequence was established. Do not claim it was
refuted globally or borrow the separate no-question guard violation. Use a rendered-token ConfirmSlot and transfer its
guard into the task. g051-L2, g056-stale, g057-controls-confirmation and g058-s2 preserve the same history and
qualification. Suggested bucket other, confirmed implementation-contract omission.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g054/g054-control-answer-binding`,
exclusive base `1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`.
Private findings history, dispositions and independent consultations preserve original confidence, suggested buckets and
every revision. No fix or triage decision was executed.
