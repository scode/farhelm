# Frozen sandboxes retain enabled header lifecycle answers

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

Retained header Restart and Replace questions, and an interrupted-session card during refresh, can keep enabled answers
for a frozen sandbox. The backend refuses those operations.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `952183b99721b44fd6fef27b5ca3b38499165b7f`. Original source location:
`crates/farhelm-ui/src/session_view.rs:2454-2498,2543-2573,2860-2921`. Locations below use that revision unless another
is named; the assessment was checked through the reviewed tip.

The main toolbar includes sandbox_frozen in ordinary action availability, but other lifecycle answers omit it. Header
Restart/Replace questions retain their slots after suspension; their confirm buttons and handlers check operation busy
state rather than frozen host state (session_view.rs:2454-2498,2543-2573). This is a stable converged-state producer,
not only a narrow refresh race.

The original interrupted-card surface also remains: its visibility checks only session terminal absence, while its
Restart/Replace controls disable only on lifecycle.busy (:1914-1928,2860-2921). Independent host/detail reads can expose
that card during the phase gap. SPEC.md:1465-1467 requires these operations disabled while suspended.

Backend routing refuses non-connected hosts before ordinary Restart/Replace mutation (helm
sessions.rs:903-924,3077,3929), so the established consequence is enabled controls and an avoidable refusal/lost
question, not automatic Resume or process loss. Include frozen state in answer availability/admission while retaining
Cancel and ownership. Suggested bucket other, confirmed source omission. The list-owned row-confirmation item is a
separate repair surface; the shared policy does not erase either location.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical
`g057/g057-discovery-interrupted-sandbox`, exclusive base `1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip
`0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings history, dispositions and independent consultations
preserve original confidence, suggested buckets and every revision. No fix or triage decision was executed.
