# Moving history pagination miscounts plan passes

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

The plan picker can count an in-flight claim as a skipped pass when main changes while history pages are read.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `6ca409d17dba9b2578e5dfd272c4e3f1fd64c164`. Original source location:
`scripts/plans-queue.py:1438-1450,1472-1477,1499; scripts/plans-queue.py:792-793,1409-1416`. Locations below use that
revision unless another is named; the assessment was checked through the reviewed tip.

Status is read from a pinned head, but history pagination follows the moving branch. If main changes between pages, a
boundary can repeat or shift, and the replay can count a claim made while the subject plan was in flight as an eligible
skipped pass. Pinning only the status read does not pin the history being interpreted.

Read every history page from one immutable revision and preserve consistent ordering across its boundaries. The
supported page-boundary replay and overdue consumer were checked by source; no live queue or GitHub history was changed.
Suggested bucket other, confirmed source confidence. This is independent of finite retention and missing
creation-boundary errors.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g007/g007-moving-history`, exclusive
base `1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private
findings history, dispositions and independent consultations preserve original confidence, suggested buckets and every
revision. No fix or triage decision was executed.
