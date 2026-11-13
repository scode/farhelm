# A sub-line preview budget can omit the whole diagnosis

Reviewed commit: 6e423e30068b62c6cfa276c6d720ee35bb70387a

## TLDR

Unverified after investigation: if a reachable short-window layout gives a host diagnostic less than one line of preview
space, the hover can omit the diagnosis entirely instead of showing its final lines. Full text remains in Hosts. The
necessary viewport/font geometry has not been established.

## Details

Suggested and recorder bucket: **other**. Confidence: **unverified after investigation**. Reviewed change:
`73754ddc87257609c8a1fd4fd4092b49591ea264`; relevant source is unchanged at the reviewed tip.

`crates/farhelm-ui/assets/tooltip.js:247-278` reserves the header, current step, section overhead and overflow notice,
divides the remaining diagnostic budget, then rounds shortened previews down to whole computed line heights. A positive
allocation smaller than one line becomes `max-height: 0px`. The prior implementation retained floored pixel height.
SPEC.md:140-144 and SPEC_impl.md:1437-1442 require a preview of whole final lines, with full details available in Hosts;
neither expressly accepts an empty preview.

Investigation covered the fitter, card sources, definite line height, wrapping CSS, attention-list scroll geometry,
native window construction/restoration and existing tests. No one-line floor or zero-allocation fallback was found. What
remains missing is a supported, reachable layout with a positive sub-line allocation while the trigger and reserved
content remain visible. CSS arithmetic alone does not establish that layout. The earlier stronger claim that some short
viewport necessarily produces it was retracted.

A renderer observation of allocated height, computed line height and connected text bounds at a reachable production
viewport would settle it. No renderer ran. Existing `short-progress-card-can-hide-overflow-notice.md` concerns fixed
chrome clipping the guidance after diagnostics shrink, not this rounding-to-zero case while guidance may fit. No exact
other acceptance, Planned item, pending repair, known bug, ledger or filter cover was established.

Provenance: trailing-review run `3033ee5b-b95a-4e16-8ecc-c4a9e727335a`, `g002/g002-l1`, exclusive base
`0bc984b71384e7b754b55d007b53ebbeb9a076ca`. Proposed drop: none; preserve the investigation and missing geometry premise
rather than claiming an observed display failure.

Current-main check (2026-11-12, `6652f84fe3ecd9427b2631fb4bca4e06fa7fc741`): tooltip.js:247-278 still subtracts fixed
content and the overflow notice, then rounds each diagnostic share down to whole line heights, permitting zero. Its only
later change is an unrelated comment at line 111. The existing short-progress-card-can-hide-overflow-notice.md concerns
fixed chrome hiding guidance rather than a diagnostic share rounding to zero. Reachable supported viewport, font and
card geometry remain unverified after investigation.
