# A very short progress card can clip its overflow notice

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

A very short update-progress hover may hide its Full details guidance even after diagnostics shrink. Reachable native
geometry remains unverified after investigation.

## Details

Suggested bucket: **other**. Recorder confidence: **unverified after investigation**. Source-confirmed interleavings are
not runtime observations.

Reviewed change: `4754d4e5f4edc8929ce77f79071fbe724d09f164`. Original source location:
`crates/farhelm-ui/assets/tooltip.js:250-272`. Locations below use that revision unless another is named; the assessment
was checked through the reviewed tip.

The floating progress card reserves its header/current row and chrome, then shrinks diagnostic previews within the
available height. With a sufficiently short viewport, fixed content can still exceed the budget after diagnosis height
reaches zero, potentially clipping the Full details guidance. Ordinary fitting is bounded when fixed chrome fits.

Investigation checked the Rust card shape, fixed current-row styling, fitting loop, notice and padding, and
viewport/font constraints. No reachable supported native geometry establishing that final clip was obtained. A geometry
and keyboard/reachability observation on that configuration would settle it; no renderer ran. Preserve accessible
overflow guidance or a reachable fallback when fixed content alone cannot fit. The scrolling-explanation filter does not
clearly apply to guidance that may be unreachable. Suggested bucket other; unverified after investigation.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g045/g045-l02`, exclusive base
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings
history, dispositions and independent consultations preserve original confidence, suggested buckets and every revision.
No fix or triage decision was executed.
