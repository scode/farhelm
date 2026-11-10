# A failed host refresh keeps a stale sandbox suspension card

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

After Resume, a failed host-list refresh can leave a recovered terminal hidden behind an unqualified Session suspended
card, with lifecycle controls disabled until a successful refresh.

## Details

Suggested bucket: **other**. Recorder confidence: **verified source assessment**. Source-confirmed interleavings are not
runtime observations.

Reviewed change: `952183b99721b44fd6fef27b5ca3b38499165b7f`. Original source location:
`crates/farhelm-ui/src/session_view.rs:1920-1928,1990-1992; crates/farhelm-ui/src/sandbox_sessions.rs:63-75`. Locations
below use that revision unless another is named; the assessment was checked through the reviewed tip.

SessionView gets sandbox phase from sidebar_hosts.hosts(), which intentionally retains the last successful rows after a
failed refresh (session_view.rs:1920-1928; hosts.rs:1472-1511). It uses that phase to force the unavailable card even
when session details are fresh (:1990-1992). sandbox_sessions::unavailable_notice wraps the old Host as Known, bypassing
HostsRead::lookup's failure-before-snapshot rule (hosts.rs:1529-1538).

A retained Suspended snapshot followed by successful Resume, failed host refresh and successful session-detail refresh
can therefore keep a recovered terminal behind an unqualified Session suspended card, with header lifecycle actions
disabled. The view does not disclose that host refresh error in this route. Existing host-lookup documentation and tests
explicitly forbid presenting stale phase as current; this new early return bypasses them.

Retain ownership/recipe facts separately from freshness-aware phase/cause lookup, and show failed/pending registry state
rather than manufacturing Known. The missing body is not proven permanent: a successful refresh repairs it. Suggested
bucket other, confirmed source state/read independence, runtime frequency unmeasured. The whole rare-glitch filter does
not clearly match because an ordinary failed registry read is sufficient; no narrow interruption window is required.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical
`g057/g057-discovery-stale-sandbox-phase`, exclusive base `1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip
`0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings history, dispositions and independent consultations
preserve original confidence, suggested buckets and every revision. No fix or triage decision was executed.
