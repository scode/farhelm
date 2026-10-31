# An early empty status filter can stay Loading

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

An early empty status-filter reply can leave the main pane saying Loading sessions until the user changes views.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The source-complete conditional reply path preserves the initial loading state instead of recording a successful empty
result. The behavior predates the reviewed change. No different operation is authorized or user work lost; no runtime
reproduction was performed.

Source locations: `crates/farhelm-ui/src/lib.rs:2040-2045; crates/farhelm-ui/src/list/view.rs:1643-1647,4528-4546`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed full tip d6645f771bee8ed03a66486ef8755834937d759d. Assigned casing commit
bd97d312165a02c351903f6bdf2dffcbe9ef8f33 preserves this pre-existing behavior; exclusive base
da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c already has the same guard at list/view.rs:1591-1595 and lowercase loading
placeholder at lib.rs:1993-1998. TLDR: on a fresh page, choose a status with no matches before the first full session
listing commits. The sidebar can finish showing its empty filtered result while the main pane continues saying Loading
sessions, indefinitely across filtered refreshes. All/default filtering or explicitly opening a session recovers.
Concrete shipped route: fleet_empty starts None (lib.rs:1810), filter starts default but is page-local
(view.rs:1284-1286), and status radios are expressly available during loading/failure (4528-4546). The onchange handler
updates status and records another listing demand (2074-2078). A pending default reply arriving afterward is rejected
before any state change because its captured query differs from current filter (1463-1480); the reader then reads the
current filtered query (1748-1815). A successful empty filtered reply is accepted but does not set fleet_empty, because
only default non-truncated replies write it (1643-1647). The auto-selection effect has no candidate and returns without
opening a session (3065-3096). No other fleet_empty writer was found in the pinned UI source. Thus the None -> Loading
branch persists despite the completed filtered request; this is not claiming a false empty fleet, which the guard
correctly avoids. Evidence checked: source producer, enabled UI control, query acceptance, next read sampling,
empty-list selection, all fleet_empty references, and historical implementation. Existing filter test first opens the
list then selects an empty host (filters.spec.ts:84-93), and placeholder-cycle test supplies an immediate unfiltered
empty reply (sidebar.spec.ts:4465-4522); neither checks this initial-filter ordering. No runtime/test/latency experiment
performed. Confidence confirmed for the conditional source route; frequency of the ordering is unmeasured. Coverage
queries across SPEC.md, SPEC_impl.md, TODO.md, BUGS.md, TRIAGE_OUTCOMES.md, plans/queue and queue INDEX found no exact
accepted behavior, scheduled fix or duplicate. SPEC.md:2090-2108 accepts filters narrowing automatic-selection
candidates; that explains no selection, not the completed request still being described as loading. Suggested bucket
other. proposed_drop false. possible_cover FILTER.md Rare, self-correcting glitches and imprecise diagnostics: this is
solely a misleading display tied to startup ordering, but the duration/rarity of an initial network read is not
established and the label does not correct on ordinary filtered refreshes. No exclusion made on that ambiguous basis. No
security, wrong-target action, data loss, or unfinished operation is alleged.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g064/f4`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
