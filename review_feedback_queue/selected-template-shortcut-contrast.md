# Selected template shortcut hint retains muted ink

Reviewed commit: 3bc9f87d59ba9b9c44ad2987e3aa16bbb365a852

## TLDR

The keyboard shortcut shown inside a selected quick-launch template is difficult to read, especially in Light
appearance. The template's main name and summary remain readable.

## Details

Suggested bucket: **other**. Confidence: **confirmed source color mismatch**; practical reading difficulty was not
measured. No wrong launch or user-work loss is alleged.

`crates/farhelm-ui/assets/app.css:3085,3101–3104,8267–8275` leaves `.templates-quick-chord` explicitly colored with
`--fg-2` inside the selected button, overriding its inverse foreground. The child is rendered at
`src/list/templates.rs:2233–2236`. Its contrast is 1.99:1 in Dark appearance, or 2.26:1 on hover. Earlier contrast was
already weak; the selection palette worsens it.

Still present at review tip `98bb15850f5f5ba1df6ed66873338261c45ff948`: CSS `3173,3189–3192,8442–8445` and template
component `2299–2302`. Light appearance produces 1.14:1, or 1.03:1 on hover. The later `font-mono` class changes no
color. The contrast test's selected-descendant bindings omit this hint.

Give the selected hint an appropriate inverse foreground and verify its actual selected and selected-hover backgrounds
in both appearances. This is distinct from the selected-state fill's contrast against its surroundings.

Possible cover: none matching in the specifications, Planned work, BUGS, queue, ledger or FILTER. Proposed drop: none.

Provenance: trailing-review run `47edc805-f43c-4ef2-abb1-0bba27a41c2f/review-98bb1585`, `g022/f2`; confirmed again
during later synthesis.
