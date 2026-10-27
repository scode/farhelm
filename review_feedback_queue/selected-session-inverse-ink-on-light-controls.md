# Selected session inverse ink reaches light child surfaces

Reviewed commit: afb41f43c8ea35c3a7333e06045ad11cf1702534

## TLDR

In Light appearance, an error inside a selected session's actions menu becomes white on white. The closed actions button
can also keep white dots when hover or focus gives it a pale background.

## Details

Suggested bucket: **other**. Confidence: **confirmed CSS cascade and rendered containment**. Actual rendering and
occurrence were not measured. No wrong-target action or user-work loss is established.

`crates/farhelm-ui/assets/app.css:373–381,8311–8316` makes `.action-error` and `.session-row-menu` white beneath a
selected row. The error actually renders inside the floating neutral menu (`src/list/row.rs:2701–2718`), whose light
background is defined at CSS `2413–2446`. The selected override therefore reaches a white child surface. Separately, the
ordinary button hover/focus background at `1966–1969` is pale, while the selected toggle keeps inverse ink.

Still present after shared-menu integration at review tip `98bb15850f5f5ba1df6ed66873338261c45ff948`:
`src/list/row.rs:2688–2705`, CSS `1966–1969,2395–2399,2473,8395–8400`. The later inset focus-ring repair does not change
these colors. Scope the inverse colors to surfaces that retain the selected fill, and give neutral child surfaces their
own appropriate foreground. Verify both the open-menu refusal and closed-toggle hover/focus states.

Possible cover: none matching in the specifications, Planned work, BUGS, queue, ledger or FILTER. The existing invalid
folder focus-outline finding has a different cause. Proposed drop: none.

Provenance: trailing-review run `47edc805-f43c-4ef2-abb1-0bba27a41c2f/review-98bb1585`, `g023/f3`; neighboring `g024`
and `g031` corroborated the child-surface problem.
