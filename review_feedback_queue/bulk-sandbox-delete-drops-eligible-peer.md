# Bulk sandbox Delete drops an eligible alternative member

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

If the first marked sandbox session retains a hidden question, bulk Delete can skip an eligible peer, leave the sandbox
untouched, clear both marks and explain only the first refusal.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `952183b99721b44fd6fef27b5ca3b38499165b7f`. Original source location:
`crates/farhelm-ui/src/list/view.rs:3939-3971; crates/farhelm-ui/src/menu_panel.rs:1124-1128`. Locations below use that
revision unless another is named; the assessment was checked through the reviewed tip.

Bulk Delete coalesces marked members to one representative per sandbox before admission (list/view.rs:3939-3971). If
that representative has an idle hidden row question, admission refuses it; an eligible peer was already dropped. The
batch clears all marks and reports not-started only for the representative, leaving the resource untouched.

The verified public sequence marks both peers first, opens Delete on the first marked row, closes its menu with that
same row's own toggle, then uses toolbar Delete. The own toggle is excluded from outside-pointer cancellation
(menu_panel.rs:1124-1128), and the question remains list-owned. The earlier direct toolbar click while the flyout stays
open is refuted because its outside-pointer relay normally cancels the question; that variant must not be used as
evidence. In-flight work separately disables bulk admission.

Choose an admissible representative or report every affected member consistently when resource deletion is withheld.
Keep the specification's once-per-sandbox deletion semantics. Suggested bucket other, confirmed corrected source
sequence with runtime occurrence unmeasured. g056-bulk-interface supplied the independently rechecked producer
correction; no duplicate defect or data-loss consequence is claimed.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g057/g057-bulk-coalesced-admission`,
exclusive base `1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`.
Private findings history, dispositions and independent consultations preserve original confidence, suggested buckets and
every revision. No fix or triage decision was executed.
