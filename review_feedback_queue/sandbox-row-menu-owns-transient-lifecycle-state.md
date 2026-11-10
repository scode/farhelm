# Closing a sandbox row menu loses its lifecycle question and outcome

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

Closing the row menu can discard a sandbox lifecycle question, its local waiter or its error. The backend operation
continues independently, so this does not establish abandoned Stop-and-suspend work.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `952183b99721b44fd6fef27b5ca3b38499165b7f`. Original source location:
`crates/farhelm-ui/src/list/row.rs:2771`. Locations below use that revision unless another is named; the assessment was
checked through the reviewed tip.

The row menu conditionally mounts its Controls component. That component owns sandbox lifecycle question state, error
state and its UI waiter, so closing the menu unmounts that ownership. The specification expects row questions to persist
independently of row/menu presentation; ordinary list-owned confirmations already use a longer-lived owner.

Move the lifecycle question, operation claim and result presentation into the persistent row/list owner, with explicit
reconciliation when its target disappears. Source inspection distinguishes UI cancellation from backend ownership: the
helm's run_owned request continues ordinary Stop followed by Suspend, so losing the local waiter does not establish
partial abandoned backend work or process loss beyond the accepted request. Suggested bucket other, confirmed
source/lifetime defect. This is separate from stale opening-token answers and from a correctly retained row question
remaining enabled on a frozen host.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g056/g056-lifetime`, exclusive base
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings
history, dispositions and independent consultations preserve original confidence, suggested buckets and every revision.
No fix or triage decision was executed.
