# Plain URL lookup repeatedly decodes full history rows

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

Hovering a URL in retained terminal history repeats full-row decoding for its characters. Added work is confirmed;
material pointer or UI latency remains unverified after investigation.

## Details

Suggested bucket: **other**. Recorder confidence: **unverified after investigation**. Source-confirmed interleavings are
not runtime observations.

Reviewed change: `befe5c72f7af72bb7e2fae339c0f4ca52ff816da`. Original source location:
`crates/farhelm-ui/assets/grid-model.js:1088-1093`. Locations below use that revision unless another is named; the
assessment was checked through the reviewed tip.

Plain-URL recognition calls raw source.line(c.abs) repeatedly while scanning characters and checking OSC precedence.
After compact binary history, historyLine materializes and validates the complete row each time. The earlier run scan
was already O(characters × runs); the newly added cost is repeated decoding, validation and allocation, not a newly
discovered asymptotic run scan.

The investigation traced the plain-URL consumer, lazy decoder, hover/row caches, maximum 1000 columns/runs and bounded
soft-wrap window. Those limits do not remove repeated materialization, but no supported-scale pointer or UI latency was
measured. A representative authorized hover measurement with matching assets and geometry would settle material impact;
runtime experiments were outside this charter. Reuse the decoded row within one lookup without assuming a persistent
cache is needed. The existing osc-admission-repeats-row-work item concerns per-OSC-candidate construction in the
container provider, a different consumer and cause. Suggested bucket other; unverified after investigation for latency.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g035/g035-l02`, exclusive base
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings
history, dispositions and independent consultations preserve original confidence, suggested buckets and every revision.
No fix or triage decision was executed.
