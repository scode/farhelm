# Successful sandbox creation loses New's recipe default

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

Immediately after opening a newly launched sandbox session, New and quick-switcher creation visibly default to Local
instead of that sandbox's recipe. Reselecting the session repairs the default.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `952183b99721b44fd6fef27b5ca3b38499165b7f`. Original source location:
`crates/farhelm-ui/src/api/sandbox_sessions.rs:81-103,121`. Locations below use that revision unless another is named;
the assessment was checked through the reviewed tip.

The dedicated create adapter reads the admitted host header but returns the bare Session unchanged
(api/sandbox_sessions.rs:81-103,121). The server qualifier enriches session references, not host projection (helm
sessions.rs:873-885,1666-1667). Ordinary create backfills that projection; the recipe branch passes the bare response
directly to on_created (create_form.rs:5613-5627 versus6589-6594).

The parent opens and retains that hostless selected-session snapshot (list/view.rs:5227-5280; lib.rs:2119-2129). A hosts
refresh cannot infer its destination; the child's detail refresh updates only child state. New starts from a fresh draft
and cannot seed the open sandbox recipe without its host binding (create_form.rs:3647-3681), so
shared::default_create_host visibly falls back to Local. Reselecting a listed session restores enriched placement.

SPEC.md:1442-1445 requires New and quick-switcher creation to preselect the open sandbox's recipe/home. Backfill a
coherent admitted host identity before using the create response as selected placement. Source confidence confirmed,
suggested bucket other: the selector visibly changes and no hidden wrong-machine create or unconfirmed sandbox deletion
is established. The backend independently refuses unconfirmed whole-resource deletion; that escalation was independently
excluded.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical
`g054/g054-discovery2-created-host-binding`, exclusive base `1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip
`0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings history, dispositions and independent consultations
preserve original confidence, suggested buckets and every revision. No fix or triage decision was executed.
