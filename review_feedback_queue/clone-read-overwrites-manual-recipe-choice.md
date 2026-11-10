# A late Clone read overwrites a manual recipe choice

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

While Clone waits for its source-host read, choosing another enabled recipe does not take ownership of the draft; the
late read can replace that choice.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `952183b99721b44fd6fef27b5ca3b38499165b7f`. Original source location:
`crates/farhelm-ui/src/list/create_form/draft.rs:2904-2911`. Locations below use that revision unless another is named;
the assessment was checked through the reviewed tip.

Clone can wait for its initial source-host read while the recipe catalog is independently ready. Choosing an enabled
recipe updates the draft but does not move CloneHostState out of Waiting or mark the choice UserTookOver. When the
source read finishes, automatic reconciliation writes the source recipe over that explicit choice.

Have a manual recipe pick acquire the same draft ownership as other user placement edits, and fence the late source
result. The investigator traced catalog readiness, the public picker, Clone state and reconciliation; no private event
construction or hidden billed launch was needed. Suggested bucket other, confirmed source route. This concerns a visible
draft being replaced, not proven loss of user-owned work. It is separate from a successful create response losing its
host projection and from generic older Clone/Replace discard decisions.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g055/g055-005`, exclusive base
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings
history, dispositions and independent consultations preserve original confidence, suggested buckets and every revision.
No fix or triage decision was executed.
