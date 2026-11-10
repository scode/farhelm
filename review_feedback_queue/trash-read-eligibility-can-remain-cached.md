# A cached eligibility check may admit Trash reads during cleanup

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

A queued Trash reader may start an extra listing while cleanup is pending. The required native event ordering remains
unverified after investigation; no destructive authority or receipt overwrite is established.

## Details

Suggested bucket: **other**. Recorder confidence: **unverified after investigation**. Source-confirmed interleavings are
not runtime observations.

Reviewed change: `b8d2fb41b0447e7619df5f1d0d341ff0167f3b5f`. Original source location:
`crates/farhelm-ui/src/list/trash.rs:713-731`. Locations below use that revision unless another is named; the assessment
was checked through the reviewed tip.

Trash read admission uses eligible.peek() on a Memo over host state and pending deletion (trash.rs:713-731). Delete
admission dirties that Memo and advances the generation synchronously (:990-991), but matching dioxus-signals 0.7.10
Memo source shows peek returns the cached value without refreshing it. A new reader normally queues after recomputation,
so the simple spawn-after-delete theory was refuted by the framework's per-scope FIFO.

The remaining conditional route needs an existing reader continuation already queued before deletion dirties the Memo.
Its old reply is correctly rejected by generation; stronger coalesced demand makes SurfaceReader loop inline into a new
attempt (reader.rs:517-535,654-665), potentially sampling the new generation with cached true eligibility. Framework
task, scope, render and desktop event dispatch were inspected, but no physical supported
confirmation/notification/response ordering establishing that interval was found. Instrumentation through shipped
scheduler/event entry points would settle it; no runtime work ran.

The possible consequence is an extra listing and host-request-gate contention during the promised pause, not receipt
overwrite, permanent polling loss or new deletion authority. g001-S6 independently confirms passive listing and deletion
remain separated; g004/g006 retain their old-generation refutations. Read current imperative dependencies or refresh the
Memo at admission if fixed. Suggested bucket other; unverified after investigation.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g005/g005-010`, exclusive base
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings
history, dispositions and independent consultations preserve original confidence, suggested buckets and every revision.
No fix or triage decision was executed.
