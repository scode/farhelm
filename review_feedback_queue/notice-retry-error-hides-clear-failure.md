# An old Retry error hides a later Clear failure

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

After Retry fails, a later failed Clear resolved action can show only the earlier Retry error.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `b8d2fb41b0447e7619df5f1d0d341ff0167f3b5f`. Original source location:
`crates/farhelm-ui/src/list/rail_notices.rs:221,235-236,276-279`. Locations below use that revision unless another is
named; the assessment was checked through the reviewed tip.

Retry completion retains its error in the rail's ui.error, and closing/reopening does not clear it. A later failed Clear
resolved correctly returns an API error and stores it in reader.action_error, but rail_notices.rs:221 selects ui.error
first with or_else. The single rendered error therefore hides the newer failed operation. Sequential failures suffice;
this needs no timing race.

API marks at api.rs:655-668 and notices.rs:131-148 propagate the failure correctly. Give each operation's result an
appropriate lifetime or select the latest relevant error, without making explicit Clear failures best-effort silence.
Suggested bucket other, confirmed source route; g001-S5 corroborates the transport-to-presentation boundary. This is
separate from notification persistence and read-generation behavior.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g002/g002-008`, exclusive base
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings
history, dispositions and independent consultations preserve original confidence, suggested buckets and every revision.
No fix or triage decision was executed.
