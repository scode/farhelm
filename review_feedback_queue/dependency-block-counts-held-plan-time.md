# Dependency abandonment counts time spent on hold

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

A plan temporarily blocked by an abandoned dependency can accumulate overdue passes from time it was explicitly on hold.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `6ca409d17dba9b2578e5dfd272c4e3f1fd64c164`. Original source location:
`scripts/plans-queue.py:1409-1416; scripts/plans-queue.py:1072-1075,1098-1130`. Locations below use that revision unless
another is named; the assessment was checked through the reviewed tip.

The pass counter follows the subject plan's transitions without reconstructing its dependency-induced eligibility. A
held plan whose parent is abandoned can become blocked, be answered back to held, and later be unheld; claims from the
held interval can then count toward its overdue threshold. The dependency transition is the missing eligibility
boundary.

Exclude claims while the plan is on hold across that blocked/answer transition, using enough dependency state to
preserve the held-time invariant. The function's documented drop-dependency approximation covers a different route and
does not explicitly accept counting held claims here. The consumer and the concrete state replay were inspected without
mutating a live queue. Suggested bucket other, confirmed source confidence.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g007/g007-dependent-transition`,
exclusive base `1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`.
Private findings history, dispositions and independent consultations preserve original confidence, suggested buckets and
every revision. No fix or triage decision was executed.
