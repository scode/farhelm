# Requester full-ID naming uses prohibited generic markup

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

The requesting session's shortened ID relies on an accessible label on a generic span, where that label is prohibited.
The markup mismatch is confirmed; actual assistive-technology output was not tested.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `c06d20d45f932746a7f042777671ddfd69d000e7`. Original source location:
`crates/farhelm-ui/src/approvals.rs:632,637,640,643`. Locations below use that revision unless another is named; the
assessment was checked through the reviewed tip.

Requester abbreviates the visible session ID and places its full display spelling only in aria-label on a generic span.
HTML-ARIA prohibits naming that span in this context, and the generic ARIA role prohibits an author-provided name. The
full raw Copy ID value and the separate approval answer target remain correct; those source refutations do not repair
this naming contract.

The exact primary references inspected were HTML-ARIA's span rule (https://www.w3.org/TR/html-aria/#el-span) and ARIA
1.2's generic role (https://www.w3.org/TR/wai-aria-1.2/#generic). Use appropriate semantics or reachable full text
instead of relying on a prohibited label. Confidence is confirmed for the standards/markup mismatch; actual native
assistive-technology output is unverified and no speech result is claimed. Suggested bucket other. This is distinct from
keyboard entry and the shared full-title hover cap.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g043/g043-requester-accessible-name`,
exclusive base `1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`.
Private findings history, dispositions and independent consultations preserve original confidence, suggested buckets and
every revision. No fix or triage decision was executed.
