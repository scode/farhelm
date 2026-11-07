# Long host name progress reachability unresolved

Reviewed commit: 1a57aa8e265cbb59cedc228433b189f59f7ad59a

## TLDR

A long host name may push progress content beyond its available layout. Actual supported native geometry remains
unverified after investigation.

## Details

Confidence: unverified after investigation; established source facts and missing premises are distinguished below.
Suggested bucket: other.

Source location:
`crates/farhelm-ui/src/list/create_form.rs:119-136; crates/farhelm-ui/assets/app.css:6305-6334 at 1a57aa8e265cbb59cedc228433b189f59f7ad59a`.

Source assessment (read with the later corrections below) (review record g058/g058-l8, event 19):

Reviewed full hash: 1a57aa8e265cbb59cedc228433b189f59f7ad59a. New discovery pass 2; investigated-unverified visual
consequence. TLDR: A valid long unbroken host name may push the Resume preparation text outside its small quick-launch
pill or scrolling launcher body, making the preparation status hard to read on narrow windows. Source facts:
HostResumeProgress in create_form.rs:119-136 puts preparation.name in a plain bdi inside a text span next to a spinner.
HostOption::display_name (list/shared.rs:423-425) calls gui_host_name (hosts.rs:319-324), which uses display_peer rather
than truncating ordinary visible names. The name comes from the host display name captured at create_form.rs:5254-5261
or 5732-5735. Valid long unbroken aliases are allowed: store.rs:1849-1867 trims, rejects controls, caps at 64
characters, and otherwise accepts the value. app.css:6305-6311 bounds the outer fixed pill to 100vw-40px; 6327-6334
makes its new Resume child a flex row with min-width:0 only on the parent. There is no child shrink/wrap/ellipsis rule
for its text span or bdi. Contrast existing launch-progress-visible/label at 6339-6349, which gives the text min-width:0
and overflow:hidden/text-overflow:ellipsis. Search of the full reviewed stylesheet for overflow-wrap,
word-break/word-wrap, bdi and the exact progress selectors found no inherited global wrap rule or applicable bdi
truncation for this component. The form body is flex-column with its own bounded scrolling area (6470-6475). Pinned tip
8a8c9579e04299410652f0e4681975a7198dfc7b:6490-6519 retains the same outer width and missing child rule. Governing
SPEC.md launch-progress paragraph (1133-1138 in reviewed commit) calls the quick-launch status a small pill at the
bottom of the window; SPEC_impl.md Tensorlake hosts (6035-6041) requires Resume preparation in that existing surface.
Confidence: likely layout bug from automatic flex min-content sizing and lack of break opportunities, but actual
clipping/overflow is unverified after investigation. Actual checks/results: inspected exact markup, the valid alias
admission, display-name path, outer/form CSS, all candidate inherited wrapping selectors and persistence at tip; no code
path truncates this ordinary visible input before render. Missing premise: rendered geometry for a permitted unbroken
wide name at an actually supported narrow viewport with the shipped font/engine, including whether horizontal scrolling
makes the whole message reachable. Source declarations expose the risk but this source-only pass did not calculate font
glyph/layout metrics or run a renderer, so it does not claim a measured viewport cutoff, hidden action or confirmed
clipping. A retained browser/desktop layout capture comparing status bounding rectangles and scroll reachability at
narrow widths would settle it; such runtime/visual experiments are prohibited. Suggested bucket other provisionally for
presentation/readability; no data/process/credential loss or submission-authority issue. Suggested fix if verified: make
the text flex child shrink with min-width:0 and wrap arbitrary names, or use the established bounded-label pattern while
retaining accessible full text. proposed_drop false; possible_cover unknown, parent handles coverage. Temporal: new
component and styling were introduced together by assigned #3048; no later repair in pinned tip. No
live-install/runtime/build/process/browser/network action.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g058/g058-l8`. The retained
private log preserves earlier wording and its corrections. This record does not claim runtime reproduction.
