# Multi-clicking the exit message copies a different row

Reviewed commit: 0592c5c6d667abe2f08706185b7566febb3434f7

## TLDR

A double or triple click on the exit message can replace the clipboard with terminal text from the row above it.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/terminal/attached.rs:4022`.

the app-drawn exit line starts at rows times cell height minus lift, has no pointer blocker, and is inserted under the
terminal body (attached.rs 2116–2150, 2337–2377, 2401–2415). GPUI dispatches the body mouse listener when its hitbox is
hovered (vendor/gpui-pre/src/elements/div.rs 159–173). A non-occluding child does not exclude it (window.rs 874–889,
1201–1221). The body starts multi-click selection through selection_position (attached.rs 4022–4040). At the exit line
its y plus lift is at or below the grid bottom, so pointer.rs 91–106 clamps it to the last text row. Model selection.rs
173–189 resolves the whole logical row for a triple click and attached.rs 4199–4213 copies nonempty selected text on
release. The parent 77b66797b218a8dae046f1d9d3da76386d6b5a60 already clamps outside presses when selecting locally, but
its mouse-reporting branch at 3974–4012 blocks ordinary local selection on a retained mouse-reporting frame. The new
after-exit mode gate now allows that branch. This conflicts with the separate exit-message row in SPEC.md tip 3302–3304
and selection from terminal text at 3467–3474. Restrict the start of a multi-click selection to the drawn text grid and
keep the intended edge clamp for active drags. Existing attached/tests.rs 3718–3737 tests multi-click copy over actual
text and 1360–1385 checks the exit line geometry and an above-grid drag. No assertion covers presses originating on the
exit message. Bucket other (wrong clipboard content). Confidence confirmed by pinned UI, geometry and clipboard source
for a visible exit line below a nonempty last row. No native gesture or clipboard result observed. Possible coverage
none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `scrollback/l12`.
