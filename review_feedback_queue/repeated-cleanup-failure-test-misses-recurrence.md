# Cleanup failure regression does not observe its repeated failure

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

The regression can pass without observing the cleanup failure it repeats, leaving recurrence handling unprotected.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `b8d2fb41b0447e7619df5f1d0d341ff0167f3b5f`. Original source location:
`e2e/tests/rail-notices.spec.ts:148-155`. Locations below use that revision unless another is named; the assessment was
checked through the reviewed tip.

The repeated-failure regression can satisfy its quiet-state assertion using the first failure's already-visible state.
It then publishes resolution before establishing that the repeated failure was consumed. The test therefore cannot
distinguish a working recurrence path from one that loses that observation. The API/storage paths' independent
durability coverage does not repair this missing browser witness.

Require an observable second failure or an application-owned processing boundary before publishing resolution, then
assert the resulting recurrence behavior. This is a source-confirmed test-sensitivity gap, with no claim that shipped
recurrence handling is broken or that a test run failed. The original reviewer suggested other with confirmed source
confidence; g001-S3 corroborates the same test defect.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g002/g002-002`, exclusive base
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings
history, dispositions and independent consultations preserve original confidence, suggested buckets and every revision.
No fix or triage decision was executed.
