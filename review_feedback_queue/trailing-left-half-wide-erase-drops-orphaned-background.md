# Erasing or deleting the left half of a colored wide glyph drops the orphaned right cell's background

Reviewed commit: a4c7c4d5e35012b5852966812f1aa6db55bd367b

## TLDR

When a terminal program erases or deletes only the left half of a wide character that has a colored background (or
inserts a blank at its right half), the surviving right cell loses its background color and paints in the terminal's
default background instead.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/grid_output.rs:163`.

Commit a4c7c4d5 fixed erasing the right half of a wide glyph by splitting in wide_glyph_owns_next_cell
(crates/farhelm-supervisor/src/grid_output.rs:205-212,249-257), but left erasing/deleting the left half broken at lines
163-169. In vendor/alacritty_terminal/src/term/mod.rs, erase_chars (lines 2015-2017, CSI X), clear_line (lines
2128,2135-2137, CSI 1 K ending on the left cell), delete_chars (lines 2037-2046, CSI P on the left cell), and
insert_blank (lines 1686-1694, CSI @ on the spacer cell) leave an orphaned cell carrying Flags::WIDE_CHAR_SPACER and the
original nondefault background (e.g., ESC[42ma界Z followed by ESC[1;2H ESC[41m ESC[X leaves col 1 with bg=Palette(1) and
col 2 with c=' ', Flags::WIDE_CHAR_SPACER, bg=Palette(2)). In crates/farhelm-supervisor/src/grid_output.rs:163-221, a
paired WIDE_CHAR_SPACER is already jumped over by col += 2 at line 221 (because wide_glyph_owns_next_cell returns true
for a paired spacer), so any Flags::WIDE_CHAR_SPACER that reaches lines 163-169 is orphaned. Because lines 163-169
unconditionally skip any cell with Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER without emitting a styled
blank run, the orphaned right cell's nondefault background is dropped and renders in the default background. Fix
direction: When an orphaned WIDE_CHAR_SPACER reaches line 163 (and likewise a shifted LEADING_WIDE_CHAR_SPACER from g26
f1), emit an unlinked 1-column space run if style_of(cell, colors) != Style::PLAIN instead of silently dropping its
style; add a unit test in crates/farhelm-supervisor/src/grid_output/tests.rs for left-half ECH/EL/DCH on a colored wide
glyph.

Reviewer metadata (g27-terminal-wide-links-erase/f2): suggested bucket: other (could not be in highest: 1-cell
background rendering mismatch after left-half erase/DCH/ICH of a colored wide character; no security or data-loss
impact); confidence: confirmed; possible cover: Related to g26-terminal-wide-proto-and-paint f1 (same skip branch at
grid_output.rs:163-169, but triggered on Flags::WIDE_CHAR_SPACER via left-half erase/DCH/ICH rather than shifted
Flags::LEADING_WIDE_CHAR_SPACER)..

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g27-terminal-wide-links-erase/f2`.
