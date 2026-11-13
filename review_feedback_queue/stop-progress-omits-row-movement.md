# Stop progress is absent from row-anchor movement inputs

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

## TLDR

Stopping an earlier session can add or remove a title line while a later session's menu or notification list is open,
leaving that popup floating at the old row position. This is conditional on noncompact row wrapping; actual native
geometry was not measured.

## Details

Suggested and recorder bucket: **other**. Confidence: **verified conditional source defect**, not an observed renderer
failure or destructive retargeting. Reviewed change: `bd2fdb7ff1afd711fa4384a8c1d14c5d67f38fc7`; references below use
the reviewed tip.

`crates/farhelm-ui/src/list/row.rs:1929,1947-1959` inserts the Stopping spinner/text before the title in the identity
copy. Noncompact copy wraps; its title has a four-glyph floor, and the progress is a separate nowrap child
(`assets/app.css:4705-4721,4748-4752,5748-5757`). Content-derived row height can therefore change when that sibling
appears or disappears. Compact copy cannot wrap and is outside this route.

The shared identity projection tracks pin/title/delete progress/bell/age but not stopping (`row.rs:74-116`;
`list/view.rs:175-184,4560-4577`), and the accepted-listing content projection tracks session detail/error text rather
than this local progress (`src/rows.rs:300-397`). Heading-only size probes and external-layout effects do not observe
that membership (`view.rs:1458-1482,1511-1532`). The user can open another row's popup while Stop waits. Successful Stop
removes stopping before awaiting its status refresh (`2516`), so progress can disappear while that popup remains open
without a new error or accepted-listing layout change. A later update may incidentally reconcile it; none is required
before this interval.

The final product contract governs the remedy: SPEC.md:3276-3283 says unrelated completions do not close the user's menu
and an open menu follows its session. Older SPEC_impl.md:1025-1027 describes closing measured popovers on layout
changes; stale coordinates satisfy neither policy. Include stopping in the shared row-flow inputs and preserve the
current opening while making its placement follow the moved anchor. Do not indiscriminately dismiss newly measured or
otherwise unaffected openings.

The marked-count reflow item concerns a synthetic bulk-toolbar anchor; the closed-popover ledger decision concerns scan
cost. Neither covers this omitted real-row contribution. No exact Planned item, pending plan, BUGS, current ledger or
filter cover was found. A future layout check can hold the post-Stop status reply, keep a later popup open with a long
earlier title, and compare row/toggle/popup rectangles. No runtime check ran.

Provenance: trailing-review run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, `g008/g008-l7`, exclusive base
`0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Proposed drop: none.

Current-main check (2026-11-12, `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`): row.rs:58-123 still projects title, pin,
delete progress, age and bell without stopping, while :2032-2037 inserts Stop progress before pin and title.
view.rs:176-185,4897-4904 still uses that projection for earlier-row movement. The conditional wrapping route remains
source-confirmed, with native geometry unmeasured. SPEC.md:3303-3311 requires independent surfaces to remain open and
follow their session; the existing marked-count item concerns its own toolbar rather than this earlier-row contribution.
