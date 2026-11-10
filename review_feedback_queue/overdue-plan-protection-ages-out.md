# An overdue plan loses protection as its claims age out

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

The plan picker can forget that a repeatedly skipped plan is overdue when older claims leave its bounded history.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `6ca409d17dba9b2578e5dfd272c4e3f1fd64c164`. Original source location:
`scripts/plans-queue.py:1370-1371,1400-1417,1438-1450,1498-1501`. Locations below use that revision unless another is
named; the assessment was checked through the reviewed tip.

The pass counter reads only a bounded 300-transition INDEX history. Claims that already made a plan overdue eventually
fall outside that window; knowing the plan's creation commit does not recover the discarded claim evidence. Its computed
pass count can fall below the protection threshold and make repeated conflict skipping available again.

The actual picker consumes that count to protect overdue plans; this is not merely historical-report imprecision.
Preserve enough history or an authoritative count/boundary to prevent already-earned protection aging out. Source replay
establishes the bounded-history mechanism; no live queue was mutated. Suggested bucket other, confirmed source
confidence. This is distinct from truncating the plan file's creation history or paginating a moving branch.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g007/g007-history-bound`, exclusive
base `1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private
findings history, dispositions and independent consultations preserve original confidence, suggested buckets and every
revision. No fix or triage decision was executed.
