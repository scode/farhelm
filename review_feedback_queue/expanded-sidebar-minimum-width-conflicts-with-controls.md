# Expanded sidebar minimum width conflicts with its controls

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The narrowest expanded sidebar leaves less room than its header and Hosts-row controls require; actual clipping and
keyboard recovery remain unverified.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

The 296px total minus 220px content reserve leaves a 76px navigation column. Header controls require 80px plus padding,
and the Hosts-row menu edge requires about 94px. CSS hidden overflow permits a reachability concern, but focus-driven
scrolling and native hit testing were not measured. Collapse and avatar context menus provide alternatives. Specified
contraction is possible coverage, without explicit acceptance of unreachable controls. This is one global width cause
across the two surfaces.

Source locations:
`plans/queue/host-rail-list-visible.md:147-154,193-201; crates/farhelm-ui/assets/app.css:623-628,5131-5197`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis source follow-up to the previous f1 event, whose detailed investigation and provenance remain applicable. At
pinned tip d6645f771bee8ed03a66486ef8755834937d759d, list/view.rs4220-4266 confirms HostsPanel shares sidebar-content
with the session header at4269-4315. Thus the296/220/76 width contract affects both surfaces, not merely session
filtering. app.css1381-1396 sets the three header buttons to min-width24px and the heading to8px padding per
side;1434-1442 makes the three-button group nonshrinking with4px gaps. This yields an80px group within60px of padded
header space, corroborating the shared cause reported by parent for g163/f2. Do not create a duplicate header finding
under g162: retain g163/f2 ownership and g162/f1 ownership if the parent synthesizes one issue. f1 remains open, bucket
other, confidence likely/unverified after investigation; source incompatibility is established, actual clipping/hit
testing and focus-induced scrolling are not observed. Missing premise and resolving evidence remain as in the preceding
event: browser geometry, elementFromPoint and keyboard-focus observation at the supported296/220/76 layout, unavailable
because runtime experiments are expressly prohibited. Product acceptance of a narrower column does not explicitly accept
loss of row/header control access. proposed_drop:null; possible_cover:SPEC.md1690-1694 narrower-column allowance, with
clipping not explicitly covered. No new security, wrong-target or user-work-loss route was found. The existing
short-height footer concern remains distinct.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g162/f1`, `g163/f2`, `g174/f1`, `g175/f1`.
These are original reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and
independent verdicts remain in the private review evidence. No fix or triage decision was executed.
