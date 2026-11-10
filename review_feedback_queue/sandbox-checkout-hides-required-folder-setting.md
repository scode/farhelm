# Sandbox checkout launch hides its required folder setup

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

A new sandbox checkout can be offered without exposing its missing global checkout-folder setting, then fail before
billing with a Settings remedy.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `952183b99721b44fd6fef27b5ca3b38499165b7f`. Original source location:
`crates/farhelm-ui/src/list/create_form.rs:4551-4569`. Locations below use that revision unless another is named; the
assessment was checked through the reviewed tip.

A recipe target has no selected installed host. The launcher's missing-checkout-root presentation checks the selected
host's observation, so a fresh recipe checkout can hide the prerequisite for the global checkout folder. Server
validation still detects it before provisioning/billing and directs the user to Settings.

Evaluate the global prerequisite for recipe placement and expose the inline setup/refusal required before an attempted
launch. The global-default dependency itself is intentional; the defect is hiding its missing setup while offering this
launch path. Source inspection confirms the predicate and backend boundary. Suggested bucket other, confirmed
confidence, without an unexpected allocation, wrong-folder launch or user-work loss claim. Pending launcher/environment
plans do not explicitly remove this prerequisite-presentation cause.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g055/g055-004`, exclusive base
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings
history, dispositions and independent consultations preserve original confidence, suggested buckets and every revision.
No fix or triage decision was executed.
