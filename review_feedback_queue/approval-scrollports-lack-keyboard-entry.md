# Approval details omit keyboard entry into their scroll regions

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

Keyboard users lack a named focus target for horizontally clipped approval facts and text-only scrolling approval
bodies. The layout test currently requires that omission.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `c06d20d45f932746a7f042777671ddfd69d000e7`. Original source location:
`crates/farhelm-ui/src/approvals.rs:528-568,643,705-708,732`. Locations below use that revision unless another is named;
the assessment was checked through the reviewed tip.

The approval card has horizontally scrolling technical facts/identifiers and a vertically scrolling fact-only body, but
neither provides the named keyboard entry needed to reach its clipped content. Copy controls are siblings, so their
focus does not supply entry into those regions. The two surfaces need correction together; they are one keyboard-entry
finding rather than duplicate discoveries.

The associated layout regression explicitly requires tabStop=false on overflowing text facts, so it would reject the
needed focusability. Correct that criterion with the markup and provide descriptive accessible names, while preserving
ordinary answer ordering and focus behavior. g043-keyboard-body supplies the vertical manifestation and
g044-keyboard-contract supplies the conflicting test expectation. Source confidence is confirmed, suggested bucket
other. No native keyboard or assistive-technology run was performed. The scrolling-explanation filter expressly excludes
content keyboard users cannot reach.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g043/g043-keyboard-facts`, exclusive
base `1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private
findings history, dispositions and independent consultations preserve original confidence, suggested buckets and every
revision. No fix or triage decision was executed.
