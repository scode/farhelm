# Tab-kind projection repeats quadratic server-list lookup on every session render

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Opening or refreshing a session with many shell tabs repeatedly scans the entire tab list just to obtain each tab's
old/new terminal label.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed source cost and
unconditional caller; supported-scale latency and any material browser hang remain unmeasured and are not claimed as
verified consequences. No security, data-loss or process-loss consequence is alleged.

Source locations: 4ed000fb454cebd9c22a88f358a93e15566b8c9e:crates/farhelm-ui/src/tabs.rs:198-216.

visible_tabs produces IDs in server order. visible_tab_infos then starts server.iter().find at index zero for each ID.
With n distinct server tabs and no optimistic changes, this performs n(n+1)/2 ID comparisons; existing visible_tabs for
that same case is linear. SessionView calls the projection unconditionally during rendering, before either
migration_case or terminal_kinds can return None. MAX_MOUNTED_TAB_ISLANDS only limits the separate terminal mount set
(tabs.rs:47-69 and session_view.rs:1628-1633); the tab strip deliberately renders every listed tab
(session_view.rs:2131-2148). TabInfo and SessionInfo.tabs do not impose a tab-count cap (proto
lib.rs:1097-1110,1743-1772), and fetch_session decodes the complete Session (UI api.rs:3204-3216).

visible_tab_infos first obtains every visible ID with visible_tabs, then server.iter().find searches from the beginning
for each ID (tabs.rs:198-216). With n server tabs and no optimistic changes, lookup comparisons are n(n+1)/2.
SessionView constructs this projection unconditionally for the rendered session (session_view.rs:1822-1831).
tabs.rs:47-69 explicitly retains all reported tabs while capping terminal islands at 32. Proto SessionInfo.tabs is an
ordinary Vec with no count check in that field (proto lib.rs:1097-1110); fetch_session decodes the session without
truncating tabs (UI api.rs:3204-3216).

Opening or refreshing a session with many shell tabs repeatedly scans the entire tab list just to obtain each tab's
old/new terminal label. This adds quadratic work even when only 32 tab terminals can be attached, and even for an
old-only host whose migration surfaces show nothing.

Possible coverage and caveats: at tip, SPEC.md:3991-4024 was read in full. 4011-4017 expressly accepts
malicious/broken-host denial of service of helm and desktop, so a native hostile-host availability escalation is not
this claim; whether browser-only hostile-input consequences are accepted is ambiguous and is not resolved here. The
retained issue is the unnecessary lookup cost on ordinary lists, not an unestablished hostile page-hang exploit.
SPEC_impl.md:2483-2502 specifies the strip projection and by-ID kinds without accepting quadratic work. TODO.md:23-25
Planned is empty. Queue typed-name-matching-repeats-quadratic-prefix-work.md was read fully and concerns template
matching, not tab projection; it is not a duplicate. TRIAGE_OUTCOMES.md:7460-7477 and 7577-7592 accept terminal-library
escape/parser costs, not this Rust metadata helper. Searches of BUGS.md, the queue INDEX, plans/queue, SPEC.md,
SPEC_impl.md, TODO.md and the ledger found no exact trigger/consequence/scope coverage. FILTER.md has no general
exclusion for deterministic repeated work; rare/self-correcting and subsecond-human-input filters do not match ordinary
session rendering.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g103/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 4ed000fb454cebd9c22a88f358a93e15566b8c9e; the
same implementation persists at ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only review; runtime
occurrence and frequency were not measured.
