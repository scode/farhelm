# An unmeasured notification panel can acknowledge unseen notices

Reviewed commit: 0bc984b71384e7b754b55d007b53ebbeb9a076ca

## TLDR

Closing a notification panel before its measured placement appears may mark unseen notices read. The required native
event ordering remains unverified after investigation; history is retained.

## Details

Suggested bucket: **other**. Recorder confidence: **unverified after investigation**. Source-confirmed interleavings are
not runtime observations.

Reviewed change: `b8d2fb41b0447e7619df5f1d0d341ff0167f3b5f`. Original source location:
`crates/farhelm-ui/src/list/rail_notices.rs:62-102,146-149,172-190,209-233`. Locations below use that revision unless
another is named; the assessment was checked through the reviewed tip.

The panel records its shown IDs while its placement is still Unmeasured and visually hidden. Closing uses those IDs to
acknowledge notices, so a close that overtakes visible placement can quiet an unseen notice while retaining its history.
The backend's exact-ID and last-visible-render rules do not determine whether that panel ever became visible.

Investigation traced panel generation, placement, close/mark callbacks and the matching Dioxus measurement paths. The
web get_client_rect path can return Ready synchronously, so an await alone is not a web scheduling witness. Desktop
measurement uses a renderer reply boundary, but no complete physical close/measurement/paint order was established.
Source inspection cannot settle that native ordering under this source-only charter. A retained renderer interaction
trace or regression through the actual measurement/close entry points would settle it. Populate acknowledgement IDs only
after visible presentation if that route is confirmed. Original suggested bucket other; current confidence is unverified
after investigation. g001-S2 shares this qualification.

Recording coverage: no exact cover was established in the applicable specifications, empty Planned bucket, unlanded
plans, known bugs, queue or triage ledger. Possible covers and their limits above remain for triage.

Provenance: trailing-review run `4c16ab16-96f7-4f02-9410-50839f5ab1f8`, canonical `g002/g002-003`, exclusive base
`1cf6e79af4548fa9158ee0f12dff6348cccdbc99`, inclusive tip `0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Private findings
history, dispositions and independent consultations preserve original confidence, suggested buckets and every revision.
No fix or triage decision was executed.
