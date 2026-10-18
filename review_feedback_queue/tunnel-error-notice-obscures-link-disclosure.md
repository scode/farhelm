# A tunnel-error notice can hide a terminal link's destination

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

A tunnel-error notice in the Mac app can hide the destination shown when hovering a terminal file or web link, while the
link remains clickable beside it. A click can therefore proceed without the full destination being visible. Actual
native rendering and harmful transfer or navigation have not been reproduced.

## Details

The notice persists until Dismiss (`crates/farhelm-ui/src/tunnels.rs:322–344`), sits beside the shell
(`lib.rs:1722–1724`), and has an opaque bottom-right box at layer 1100 (`assets/app.css:7754–7765`). Disclosures remain
at layer 40 (`:1263–1280`). The inspected ancestors supply no stacking context reversing that order.

A link above or left of the notice can remain clickable while its below/right disclosure extends beneath the notice. For
illustration, a 400×60 notice at (784,724) in a 1200×800 window can cover a 350×70 disclosure at (790,714) from a
pointer at (778,698). These are feasible example dimensions, not a rendered measurement or evidence of terminal-cell
placement.

At the reviewed commit, file admission checks size, overflow and viewport containment
(`terminal-files.js:12–19,419–445`), while hovered web-link activation has no occlusion check
(`terminal-links.js:135–171`). An uncovered terminal press/release passes the integration gate
(`terminal.js:5493–5530`). The notice intercepts hits inside its rectangle, not those outside it. Plain URLs without a
hover remain excluded under the recorded policy; relevant web cases are program-emitted hyperlinks and plain URLs
showing tunnel hints, including Cmd-click tunnel opens.

Refusal explanations can also be hidden by a tall notice or short terminal. Their original layer was 41
(`app.css:1333`); raising them only above approval cards does not put them above this notice. Include the notice in
click-time disclosure visibility and keep any refusal explanation visible. A native reproduction should establish actual
overlap, then verify that a covered file/web disclosure admits no transfer or opening while an uncovered one retains its
ordinary behavior.

Suggested bucket: conservatively **highest**, because destination disclosure is part of link admission. Confidence:
**likely**; layering, persistence, event admission and feasible geometry are corroborated, while native rendering and
harmful transfer/navigation remain unverified. No arbitrary execution or exfiltration is claimed. Proposed drop: none.
Provenance: persistent Astra/high review, `browser-links-and-files-1/f1`, `ui-layout-1/f1`, and
`browser-event-integration-1/f3`. The initial review recorded this as an extension to
`approval-cards-can-obscure-file-disclosure.md`; this standalone record preserves that extension after the original
approval-card finding was fixed and removed.

Possible cover: `TRIAGE_OUTCOMES.md:15549–15590` accepted an approval-card intersection check and explicitly returned
broader occlusion work for a separate decision. The tunnel notice falls outside that implementation scope. No matching
Planned item, BUGS entry or filter covers the retained security uncertainty.

Rebase check at `084bd50201a9` after the approval-card fix `7eeb5ff87c68`: file and hovered web-link clicks now share
`admitDisclosure`, but the supplied covering rectangles still come only from `.approval-cards`
(`terminal.js:3587–3590,3613–3614`; `terminal-files.js:365–370`). The tunnel notice is not included. Refusal messages
now use layer 46, still below the notice's 1100. The updated SPEC.md requires a fully visible disclosure when another
Farhelm surface covers it; the landing report for `covered-link-hover-refusal` expressly retains the narrower
approval-only implementation. That fix resolves the old approval-card finding, not this remaining notice surface.
