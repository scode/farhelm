# Progress hover retains the abbreviated host name

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

Update progress repeats a shortened host name instead of revealing its full display name, so similarly named hosts
remain indistinguishable there.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `4754d4e5f4edc8929ce77f79071fbe724d09f164`. Original source location:
`crates/farhelm-ui/src/hosts.rs:4031`. Locations below use that revision unless another is named; the assessment was
checked through the reviewed tip.

The shared progress-card source repeats the shortened host label instead of the existing full GUI display name. Hosts
whose DNS names share the first label can remain indistinguishable in the detail hover, despite that hover's full-name
purpose. The shared source affects both progress surfaces.

Use the established full-name display helper, preserving its local-host and explicit-alias policy, presentation escaping
and direction isolation; do not concatenate raw destination text as a substitute. The investigation traced the shared
Rust producer and its consumers, rather than treating the sidebar label alone as an error. Suggested bucket other,
confirmed source confidence. This is not the older full-host-tooltip line-cap item: the value is already abbreviated
before layout, and no extreme length or geometry is required.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g045/g045-f03`, exclusive base
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings
history, dispositions and independent consultations preserve original confidence, suggested buckets and every revision.
No fix or triage decision was executed.
