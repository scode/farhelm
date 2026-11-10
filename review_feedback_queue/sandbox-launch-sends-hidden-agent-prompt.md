# Sandbox launch sends a hidden unsupported-agent prompt

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

Switching from an agent with Initial prompt to Goose or Pi can hide a retained prompt that still blocks sandbox launch.
The backend refuses before billing.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `952183b99721b44fd6fef27b5ca3b38499165b7f`. Original source location:
`crates/farhelm-ui/src/list/create_form.rs:5577-5584`. Locations below use that revision unless another is named; the
assessment was checked through the reviewed tip.

Enter Initial prompt for a harness supporting it, then select Goose or Pi. The editor hides but retains the raw prompt.
The recipe branch sends current.prompt instead of the ordinary active_prompt projection, so server validation rejects
the unsupported prompt before billing. The hidden value is difficult to clear without switching back.

Use the active prompt semantics in the recipe request or consistently clear dormant prompt state on the relevant agent
transition, preserving ordinary draft retention where intended. The investigation checked the enabled transition, both
request-building branches and server refusal. Suggested bucket other, confirmed source confidence; no allocation, lost
session or work-loss consequence is claimed. This is an ordinary launch refusal, outside the filter for rare
non-first-class-harness diagnostics.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g055/g055-006`, exclusive base
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings
history, dispositions and independent consultations preserve original confidence, suggested buckets and every revision.
No fix or triage decision was executed.
