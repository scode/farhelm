# A retained save error can consume the attention list's height

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

A retained update-preference error and wrapped header may leave no reachable attention-list body. The required supported
viewport and font geometry remain unverified after investigation.

## Details

Suggested bucket: **other**. Recorder confidence: **unverified after investigation**. Source-confirmed interleavings are
not runtime observations.

Reviewed change: `c74be1ab43dd97fbdc4287ea5e9b2a37729d87c4`. Original source location:
`crates/farhelm-ui/assets/app.css:10115`. Locations below use that revision unless another is named; the assessment was
checked through the reviewed tip.

The attention surface retains a failed fixed-preference save outside its scrolling body. The header may wrap, and the
whole surface is capped at 45% height; their combined fixed content can consume the body's viewport while overflow
prevents access. This is separate from fitting the floating progress tooltip.

Source inspection covered the cap, flex/overflow rules, retained error lifetime, width/font inputs and changing fleet
content. Update buttons disappear on the initial successful update path, but later fleet offers can coexist with the
retained error. No exact supported native viewport/font/content geometry producing an unreachable body was established.
A retained geometry/keyboard reachability check would settle it; no renderer experiment ran. Preserve a reachable
minimum scrolling body or move fixed error content within reachable scrolling if confirmed. The dialog-explanation
filter requires content to remain reachable and therefore does not clearly cover this concern. Suggested bucket other;
unverified after investigation.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g042/g042-fixed-error-body-budget`,
exclusive base `1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`.
Private findings history, dispositions and independent consultations preserve original confidence, suggested buckets and
every revision. No fix or triage decision was executed.
