# Launcher target hover merges separately isolated values

Reviewed commit: da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c

## TLDR

The launcher's new target hover combines host and folder into one text string. Legitimate right-to-left names could
appear grouped differently there than in the visible summary, which keeps the values separate. The source difference is
confirmed; actual misleading rendering or a harmful launch is **unverified after investigation**.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation** for the presentation consequence. A complete
misleading-target route causing a security or work-loss consequence would warrant `highest`; no such route was shown.
The composite hover was introduced by `09b806f7f08f436afacf0ac1f7cb867c8a7ef126`.

At the reviewed tip, `crates/farhelm-ui/src/list/create_form.rs:5910–5923` joins agent, host and folder with middle-dot
separators. At `7447–7453` that string becomes `data-tooltip`, while the inline host and folder retain separate
`dir="ltr"` `.peer-value` spans. The shared renderer at `assets/tooltip.js:299–308,332–338` assigns ordinary tooltip
text through one `textContent` value. `assets/app.css:1604–1627` isolates the tooltip as a whole; it cannot reconstruct
the original per-value boundaries. The `.peer-value` rule and its explanation at `5025–5033` preserve those boundaries
in the inline summary.

The investigation checked that this is not limited to rejected input. `src/peer.rs:159–179` escapes unsafe format
characters while preserving ordinary RTL letters, and both alias validators accept those letters
(`src/hosts.rs:3240–3253`, `crates/farhelm-helm/src/store.rs:1582–1601`). For example, a host named `אב` and folder
named `גד` produce the logical string `Codex · אב · גד`. No actual rendering of that string was measured. Escaping
format controls and using `textContent` still prevent the examined control-character and markup-injection mechanisms;
they do not establish the visual order of separate ordinary RTL values in one run.

The remaining premise is a supported native rendering that changes the apparent grouping or role of those values
relative to the isolated inline summary. A matching retained capture or focused rendering check would settle that
premise; a stronger consequence additionally needs harmful reliance. This source-only review ran no reproduction. If
confirmed, a tooltip card preserving per-value spans is a possible correction.

Possible cover is ambiguous: `SPEC_impl.md:1115–1154` permits escaped text tooltips, but does not explicitly accept
value-role changes in composite text. `SPEC.md:3421–3430` concerns escaping unsafe peer text, not removal of ordinary
RTL letters. The decisions at `TRIAGE_OUTCOMES.md:2287–2330,16420–16438` concern raw controls and different surfaces. No
exact Planned item, unlanded plan, BUGS entry or queue item was found. Proposed drop: none.

Provenance: trailing-review run `2b7a8eaf-c4e9-42ab-be93-1acf29106132`, discovery finding `g05/f6`. The parent checked
the composite value, tooltip renderer and cited coverage; observed rendering remains outside the evidence.
