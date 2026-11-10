# Sandbox Launch bypasses its repository-search refusal

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

Sandbox Launch can proceed with the previous repository or home destination while the form says Select a repository for
the unfinished search. With creation questions skipped, it can allocate a billed sandbox.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `952183b99721b44fd6fef27b5ca3b38499165b7f`. Original source location:
`crates/farhelm-ui/src/list/create_form.rs:5503-5640`. Locations below use that revision unless another is named; the
assessment was checked through the reviewed tip.

An unfinished gh: search can leave the previous accepted repository or home destination in the draft. The visible button
shows the Select repository refusal, but submit still reaches the recipe early-return branch before the ordinary
live-query guard. The retained destination can therefore be dispatched; when the user skips sandbox creation questions,
this can allocate a billed sandbox.

Apply the same unaccepted-search admission guard before recipe dispatch as before ordinary launch; disabled styling
alone is insufficient. The investigation traced the displayed refusal, submit handler, both branches and the
skip-confirmation path. Source confidence confirmed, suggested bucket other; no destruction or security consequence is
imported. The older repository-selection-keeps-global-search-launch-block ledger outcome repaired an explicit accepted
selection retaining its stale query, the opposite condition; it does not cover submitting a still-unselected query here.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g055/g055-007`, exclusive base
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings
history, dispositions and independent consultations preserve original confidence, suggested buckets and every revision.
No fix or triage decision was executed.
