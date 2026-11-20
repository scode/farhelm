# A sign-in popover can bypass its attention row

Reviewed commit: de61bc055404a995a832cd3d573f54440b6c5954

## TLDR

If the sidebar Answer button is below the fold, an automatic sign-in popover can open beside the selected session’s card
instead.

## Details

The same Action is rendered in sidebar attention and in the unavailable session card. Both defer consideration with
their local visibility and anchor in `crates/gui/farhelm-gui/src/sign_in.rs:482–522`. The clipped sidebar action asks to
scroll while the visible session-card action can open the shared popover, after which the sidebar cannot open another
one. SPEC.md places automatic opening beside the attention row. Use that anchor for the automatic case and retain the
drawn anchor for direct activation. The route is supported by source. Placement was not measured.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `host-attention/l5`.
