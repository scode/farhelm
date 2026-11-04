# Unverified after investigation: physical-budget impact of confirmed URI capacity under-accounting

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Hyperlink allocations can be larger than the terminal memory estimate counts, although the logical storage counter still
obeys its cap.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed
under-accounting of retained String capacity for Rust 1.99.1; unverified after investigation as to material availability
harm or breach of the approximate total resident-memory budget. Exact remaining premise: whether the library/docs mean
their 64 MiB physical-allocation component as a hard contract rather than the implemented logical charge, and whether
the approximate 190 MiB figure is exceeded by a permitted combined state; no measured resident-memory harm is claimed.
Contract checks: tip term/mod.rs:72-101 explicitly defines MAX_EXTRA_STORAGE_BYTES as what ExtraStorage counts and
HYPERLINK_COST as an estimate. vendor/README.md:101-140,822-859 likewise specifies length-based charges. In contrast
crates/farhelm-terminal-emulator/src/lib.rs:38-84 and SPEC_impl.md:2588-2597 use 64 MiB extras in a physical total and
promise at least five hostile terminals per GiB.

Source locations: vendor/alacritty_terminal/src/term/cell.rs:135-142.

vendor/vte/src/ansi.rs:1449-1452 builds a URI by growing a String once per semicolon-delimited segment.
vendor/alacritty_terminal/src/term/cell.rs:135-142 charges that URI's length, then :279-290 retains the moved String
without normalizing capacity. HYPERLINK_COST in vendor/alacritty_terminal/src/term/mod.rs:98-101 is a fixed 128-byte
estimate for allocation and rounding; it does not scale with spare URI capacity. Repeated distinct OSC 8 ids can retain
these allocations. The counter is still bounded; no unbounded allocation or loss-of-work route was established.

vendor/vte/src/ansi.rs:1449-1452 builds a URI from the first semicolon-delimited segment then repeatedly grows that
String with push/push_str. vendor/alacritty_terminal/src/term/cell.rs:135-142 moves that String into HyperlinkInner but
charges uri.len(), not uri.capacity(); :279-290 keeps the moved String unchanged. The fixed HYPERLINK_COST estimate in
vendor/alacritty_terminal/src/term/mod.rs:98-101 accounts for allocation and rounding, not potentially kilobytes of
unused capacity from growth. An allowed URI close to 2 KiB made by extending a large initial segment can retain capacity
substantially beyond its length; many distinct OSC 8 ids retain those allocations independently.

at the tip vendor/vte/src/ansi.rs:1449-1452 copies the first URI segment into a String then grows it with push/push_str
for later semicolon-separated segments. vendor/alacritty_terminal/src/term/mod.rs:56-70,2077-2098 caps the finished URI
at 2083 bytes and explicit id at 250 bytes, reuses only an unchanged explicit active link, and otherwise calls
ExtraStorage::link. vendor/alacritty_terminal/src/term/cell.rs:135-142 charges id.len()+uri.len()+128, :279-290 moves
the URI into the retained link unchanged, and :61-79 clones only its Arc. No shrink or capacity charge intervenes.
Version-matched static dependency check: installed official Rust 1.99.1 source (compiler 58cf8f9c3, manifest dated
2026-10-15), library/alloc/src/string.rs:2928-2932,3074-3110,3141-3153 routes str.to_string through
String::from/to_owned; alloc/src/str.rs:246-253 copies the byte slice; alloc/src/slice.rs:444-459 requests the slice
length. string.rs:1432-1442 sends push through reserve; :1235-1237 sends reserve to Vec; alloc/src/vec/mod.rs:1460-1462
reaches RawVec; alloc/src/raw_vec/mod.rs:503-535 doubles existing capacity when growing. Thus a first segment close to
the accepted URI-length limit can be doubled by a delimiter append while the completed URI still satisfies the 2083-byte
cap. That spare capacity survives the move into HyperlinkInner. This is static source behavior, not an unavailable
runtime premise. Mutation/sharing: tip cell.rs:147-247 gates copies and refunds charges on drop; link and cell clones
share Arcs, so the slack is retained per distinct link, not multiplied per cell sharing one link. Producer caps keep the
slack bounded, not an unlimited allocation route.

hyperlink allocations can be larger than the terminal memory estimate counts, although the logical storage counter still
obeys its cap.

Possible coverage and caveats: SPEC.md:3991-3994 accepts supervisor availability effects from hostile same-account
processes; :4019-4024 and TRIAGE_OUTCOMES.md:7594-7643 accept old viewer-library exhaustion, which is a different
process/scope and not full coverage of this arithmetic. TODO Planned is empty; searches of SPEC.md, SPEC_impl.md,
TODO.md, BUGS.md, plans/queue, queue INDEX and TRIAGE_OUTCOMES.md found no matching accepted retained-capacity
mechanism. FILTER.md has no clear full match.

Additional reviewed locations: g064/f1: vendor/alacritty_terminal/src/term/cell.rs:135-142.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g065/f2`, `g064/f1`.
Original append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs.
Locations refer to the named reviewed revisions; the original revision context is
c345799ed43ddbd6e0c577f8f2a0add4dd051c6a; persistence checked at ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a
source-only review; runtime occurrence and frequency were not measured.
