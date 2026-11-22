# Erasing half of an inverted, decorated, or linked wide glyph keeps stale attributes on the erased cell

Reviewed commit: a4c7c4d5e35012b5852966812f1aa6db55bd367b

## TLDR

When a terminal program erases just the right half of a wide character that is reverse-video, underlined, struck
through, or part of a hyperlink, using the same underlying background color, Farhelm keeps painting the old inverted
background or text decoration across the erased column and still treats the erased column as clickable for that
hyperlink.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/grid_output.rs:249`.

In crates/farhelm-supervisor/src/grid_output.rs:205-212 and 249-257, row_from_cells passes only style.bg to
wide_glyph_owns_next_cell(&row[Column(col + 1)], style.bg, colors). When the right cell no longer has
Flags::WIDE_CHAR_SPACER (for example after ECH CSI X or EL CSI K, which set *cell = bg.into() with Flags::empty() and no
hyperlink in vendor/alacritty_terminal/src/term/mod.rs:2016,2136), the fallback branch returns true whenever next is a
plain space with no flags (apart from WRAPLINE), no zerowidth marks, no hyperlink, and wire_color(next.bg, colors) ==
style.bg. This ignores three attributes on the wide cell itself:

1. Reverse video (Flags::INVERSE / style_flags::INVERSE): crates/gui/farhelm-gui/src/terminal/style.rs:64-68 swaps fg
   and bg when INVERSE is set, so the wide cell's painted background is style.fg (or default foreground), whereas the
   erased right cell is un-inverted and painted with next.bg (or default background). Writing an inverted wide glyph
   with default colors (ESC[7m界) and erasing its right half with default background (ESC[1;2H ESC[0m ESC[X) compares
   Color::Default == Color::Default -> true, emits width=2 with INVERSE, and paints the erased cell in the inverted
   foreground color instead of the erased default background. The same happens with explicit colors (e.g. ESC[7;31;42m界
   erased with ESC[0;42m ESC[X paints red instead of green, and erasing with ESC[0;41m ESC[X splits unnecessarily even
   though the visible background is red).
2. Underlines and strikethrough: when the wide glyph has any underline flag or STRIKETHROUGH, emitting width=2 causes
   paint_decoration (crates/gui/farhelm-gui/src/terminal.rs:1293) to draw the underline or strikethrough across the
   erased right cell whose flags were cleared.
3. OSC 8 hyperlinks: when the wide glyph has link.is_some() and its right half is erased with the same background color
   (including default background), emitting width=2 carries the wide cell's link across both columns, so
   TerminalLink::covers (crates/gui/farhelm-grid-model/src/links.rs:59-63) includes the erased column even though the
   emulator cleared next.hyperlink(). The existing test right_half_erase_after_a_wrap_preserves_the_previous_padding
   (crates/farhelm-supervisor/src/grid_output/tests.rs:307-349) only exercised a changed background (41m vs 42m). Fix
   direction: Pass style and link.is_some() to wide_glyph_owns_next_cell and require link.is_none() and no INVERSE,
   underline, or STRIKETHROUGH flags on the wide cell before absorbing a non-spacer blank cell (or, for INVERSE with
   concrete non-default fg/bg, compare against the effective painted background while still splitting when INVERSE uses
   Color::Default). Add unit tests in crates/farhelm-supervisor/src/grid_output/tests.rs covering right-half erase of
   inverted, underlined/struck-through, and same-bg hyperlinked wide glyphs.

Reviewer metadata (g27-terminal-wide-links-erase/l3): suggested bucket: other (could not be in highest: affects 1-cell
rendering and OSC 8 hit area on an erased half-cell, while OSC 8 hover disclosure remains required before any click
opens a URL; no security bypass or loss of user-owned work); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g27-terminal-wide-links-erase/l3`.
