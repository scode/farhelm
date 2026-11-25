# A wide character next to an unrelated leftover spacer takes over that cell's background

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

When a program deletes characters starting halfway through a wide (CJK or emoji) character, the cell left behind can
show the wrong background color in Farhelm. This is the class of bug the reviewed fix set out to fix, but one
arrangement is still missed. The effect is cosmetic: one cell's color. Well-behaved line editors do not do this; it
takes a program that is not wide-character aware, or one that miscounts widths.

## Details

Pre-existing. 3e7069a7eb ("fix: preserve backgrounds on isolated terminal spacers") documents the remaining case and
leaves it unfixed. Main references: `crates/farhelm-supervisor/src/grid_output.rs:277-290` (`wide_glyph_owns_next_cell`,
first disjunct) and `:226-236` (the width-2 decision in `row_from_cells`).

The commit adds a branch so that a wide-character spacer whose glyph is gone still sends its own background
(`grid_output.rs:162-192`). That branch runs only for spacers no glyph consumed. A wide glyph consumes the next cell
whenever that cell carries `WIDE_CHAR_SPACER` (`:278-279`), without checking that the spacer belongs to it. The new doc
comment says so: "The emulator's adjacent spacer flag supplies the pairing. Cell moves can leave unrelated cells
adjacent." alacritty's `delete_chars` (`vendor/alacritty_terminal/src/term/mod.rs:2021-2047`) and `insert_blank`
(`:1670-1695`) move cells without repairing wide pairs.

A concrete route, reasoned from source and not executed: print `界` with the default background, then `\x1b[42m界`
(green), giving G1 at col 0, its spacer at col 1, G2 at col 2 and its green spacer S2 at col 3. Then `\x1b[1;2H\x1b[2P`
deletes two cells starting at the spacer column. `delete_chars` swaps row[1] with row[3], so col 0 is G1 (default
background) and col 1 is S2 (`WIDE_CHAR_SPACER`, green). `row_from_cells` sees G1 as wide with next =
`WIDE_CHAR_SPACER`, so it emits one width-2 run in G1's style. Col 1's green background never reaches the client, which
paints col 1 with G1's default background. alacritty's own renderer would paint col 1 green behind the overhanging
glyph.

Verify by following `delete_chars` with those inputs, then the loop in `row_from_cells`. Fix sketch: when the spacer's
background, or its INVERSE paint, differs from the glyph's run, end the glyph at width 1 and let the new isolated-spacer
branch emit the spacer, as the erased-right-half path already does for a differing background. The doc comment's worry
about variation-selector widening applies only when the glyph's own style differs, so compare the spacer's paint
(background, or foreground under inverse), not the whole style.

Reachability: well-behaved line editors delete a wide character from its left column, which keeps pairs intact. Starting
a DCH or ICH on a spacer column takes a program that is not wide-character aware, or one that miscounts widths.

Bucket: other (one cell shows the wrong background color; no data, security or interaction consequence, since a link on
the glyph already covers both columns visually). Not highest: only paint color changes, and the run carries the glyph's
own link, which the glyph visibly covers.

Confidence: likely. Unverified premise: that alacritty leaves the moved spacer's `WIDE_CHAR_SPACER` flag and green
background intact after `delete_chars`; the swap-only implementation at `mod.rs:2037-2039` indicates it does. No
reproduction was run.

Possible cover: SPEC.md's "Terminal experience" fidelity bullet (SPEC.md:3351-3354) accepts "wide-character edge cases"
but lists them with "only ...", and this case is not in the list. The new doc comment at `grid_output.rs:272-273`
acknowledges the adjacency but does not say the background loss is accepted.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g11 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
