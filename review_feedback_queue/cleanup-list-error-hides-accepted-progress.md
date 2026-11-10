# A retained listing error hides accepted cleanup progress

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

Retry can accept cleanup while Trash keeps showing its earlier listing error without running progress, until a
successful refresh arrives.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `b8d2fb41b0447e7619df5f1d0d341ff0167f3b5f`. Original source location:
`crates/farhelm-ui/src/list/trash.rs:501-510`. Locations below use that revision unless another is named; the assessment
was checked through the reviewed tip.

A capable host can accept cleanup while the client holds a listing Err that committed before admission. Receipt handling
returns through that cached-error branch before marking the selected entries removing. The request and explicit
follow-up demand remain owned, but progress is missing until a successful listing replaces the error.

The investigation distinguished this committed-error path from an old in-flight reply: mutation generations still reject
stale replies, and read demand is not permanently lost. Preserve the receipt's accepted-work/progress state
independently of whether the last listing succeeded, while keeping the error visible. Source confidence is confirmed,
suggested bucket other. g001-S1 is corroboration. No permanent polling failure, unauthorized deletion or measured
occurrence is asserted; the rare-glitch filter's full trigger is not established.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g005/g005-004`, exclusive base
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings
history, dispositions and independent consultations preserve original confidence, suggested buckets and every revision.
No fix or triage decision was executed.
