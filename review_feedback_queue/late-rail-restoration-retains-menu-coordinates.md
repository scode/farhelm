# Late rail restoration may retain old menu coordinates

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A host menu opened before saved expansion finishes restoring may remain at the host's former position.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

Matching Dioxus source appends the sidebar script without awaiting its load; the supported web path permits restoration
after menu measurement. The menu has no script-readiness gate or independent geometry observer for this change. The
remaining premise is whether the measured menu survives all existing scroll/resize invalidators in a concrete delivery
sequence. Rarity and whole-consequence coverage are not established, so the rare-display filter is not applied. IDs
remain bound; no additional harmful action route is claimed.

Source locations:
`crates/farhelm-ui/assets/sidebar-width.js:69-74,171-173; crates/farhelm-ui/src/hosts.rs:728-755,2585-2586,4158-4177`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Static-order disposition check complete at reviewed tip d6645f771bee8ed03a66486ef8755834937d759d. Preserve g174/f2.
Bucket OTHER, proposed_drop none. The supported web loading order does not refute menu-before-restoration:
SIDEBAR_WIDTH_JS is an ordinary asset! declaration (lib.rs1446), unlike appearance's with_static_head flag(1462-1464),
rendered through document::Script at1856 beside a preferences-gated UI that does not await it(1863-1872). Cargo.lock
pins dioxus-document/web/desktop0.7.10. Matching local primary source shows Script creates its head component and
returns VNode::empty (dioxus-document src/elements/script.rs92-117); web create_script queues a DOM element append and
does not await load (dioxus-web src/document.rs137-145,167-185). The host avatar request/keyboard/context handlers
(hosts.rs728-755) have no sidebar-width readiness gate; only the rail-toggle and separator binders wait for that owner
(app_bar.rs924-930; lib.rs2011-2017 read previously). A delayed browser asset response can therefore be concurrent with
a usable measured menu; no startup ordering barrier was found. Native inclusion checked: declare_assets! builds the
shared all_assets list(lib.rs1185-1206), the embedded handler serves requested bytes
synchronously(desktop/assets.rs45-78,139-163), and native_head inlines only appearance+CSS(desktop.rs270-292), not
sidebar-width. Native Dioxus create_script queues eval of a dynamically appended script (dioxus-desktop document.rs41-59
and dioxus-document document.rs36-49/head.js1). Inclusion ensures bytes are present, not a menu readiness barrier. No
claim that ordinary native local service actually delays long enough; browser is the supported conditional producer.
Remaining missing premise: the measured menu must survive restoration without an engine scroll/resize or another
existing invalidator closing it. Actual visual/event outcome not observed; framework/local source checks cannot supply
that event trace. A future authorized browser observation with a delayed asset response at the capped width would settle
it. Rarity and the entire consequence needed for FILTER.md remain unestablished, so no filter drop. Earlier specific
menu title/id/binding observations remain limited to those controls; explicit IDs are not a general consent waiver. No
additional harm route asserted or investigated.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g174/f2`, `g175/f3`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
