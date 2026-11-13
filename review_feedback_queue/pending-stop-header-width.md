# Pending Stop may exceed the compact header's width

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

## TLDR

Unverified after investigation: the spinner and Stopping… label may push some session-header controls outside a 650px
pane when a tunnel pill is present. Actual compact-control geometry has not been measured.

## Details

Suggested and recorder bucket: **other**. Confidence: **unverified after investigation**. Reviewed changes:
`bd2fdb7ff1afd711fa4384a8c1d14c5d67f38fc7` and `8f1e23bbe8a175e25165afe90ece1f4e774ec778`. References below use the
reviewed tip.

`crates/farhelm-ui/src/session_view.rs:2373-2377` adds the pending label/spinner. `assets/header-layout.js:18-42,49-51`
measures normal actions and observes width changes, including this label: there is no missing-observer bug. The
remaining concern is the compact state's minimum total width. `assets/app.css:2072-2119,9746-9760` retains a
nonshrinking action group, 112px tab floor, 154px single-mapping tunnel floor, header/control gaps and compact labels;
the spinner adds 12px (`5758-5770`). Exact glyph advances, borders and flex layout are unresolved renderer inputs.

SPEC.md:2931-2937 and SPEC_impl.md:717-728 require the persistent controls and tunnel pill visible at 650px; SPEC.md:
1840-1844 also requires pending Stop feedback. Existing header tests cover steady Stop/tunnels, not their pending
combination. Completion tests hold Stop without that geometry. The tunnel-spacing queue item concerns confirmation
button padding, not this compact-width limit. No exact acceptance, Planned item, unlanded repair, BUGS, ledger or filter
cover was established.

A future authorized check should hold a Stop reply with visible tabs and one tunnel mapping in a measured 650px pane,
wait for compaction, then compare persistent-action/port rectangles with pane bounds using the production font. That
would establish or refute overflow; no browser/native layout ran in this review. No ordinary-window clipping, lost
controls or harmful action is asserted as observed.

Provenance: trailing-review run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, `g002/g002-l2`, exclusive base
`0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Proposed drop: none for the investigated uncertainty.

Current-main check (2026-11-12, `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`): session_view.rs:2382-2386 still replaces
Stop with a spinner and Stopping. app.css:9671 retains the 112 px tab floor, :9684 retains the 154 px mapping floor, and
:9677-9681 compacts the same action group. SPEC_impl.md:719-747 still requires the actions and tunnel pill to fit at 650
px. The existing tunnel-spacing-shrinks-confirmation-buttons.md concerns confirmation padding rather than pending-action
width. The observer exists; actual minimum glyph widths and resolved flex geometry remain unverified after
investigation.
