# A very large notification history exceeds its mark-request budget

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

Extremely large notification histories may become impossible to acknowledge or clear in the app. Supported lifetime size
and successful rendering at that scale remain unverified after investigation.

## Details

Suggested bucket: **other**. Recorder confidence: **unverified after investigation**. Source-confirmed interleavings are
not runtime observations.

Reviewed change: `b8d2fb41b0447e7619df5f1d0d341ff0167f3b5f`. Original source location:
`crates/farhelm-ui/src/api.rs:655-668; crates/farhelm-ui/src/list/rail_notices.rs:209-216,276-279; crates/farhelm-helm/src/lib.rs:701-702`.
Locations below use that revision unless another is named; the assessment was checked through the reviewed tip.

The panel serializes all shown or resolved notice IDs in one mark request (rail_notices.rs:209-216,276-279;
api.rs:655-668), while the read query has no lifetime row limit and resolution retains rows (helm
notices.rs:95-120,204-210,252-261). The mark route has no body-limit override. Lockfile-matched axum 0.8.9/axum-core
0.5.6 source confirms the ordinary Json/Bytes extraction limit is 2,097,152 bytes; the initially suspected 64 KiB limit
belongs to recipe writes instead.

The conditional failure needs roughly hundreds of thousands of ordinary integer IDs. Whether legitimate accumulated
history reaches that magnitude within supported use, and whether the shipped renderer can display it sufficiently to
dispatch the controls, remain missing premises. The fleet's tens-of-sessions scale does not explicitly bound lifetime
notice history. No large request was constructed or sent. A supported-history decision or an authorized observation of
an actual collection and mark result would settle it. Bound history consistently or chunk exact-ID marks if retained.
Suggested bucket other; unverified after investigation for supported manifestation, with the source size mismatch
established.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g001/g001-D4`, exclusive base
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings
history, dispositions and independent consultations preserve original confidence, suggested buckets and every revision.
No fix or triage decision was executed.
