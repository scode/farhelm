# Deleting a wide character's spacer swallows a shifted space with the same background

Reviewed commit: a4c7c4d5e35012b5852966812f1aa6db55bd367b

## TLDR

If a terminal program deletes only the right half of a wide character and that shifts a following space into its column
(with the same background color), Farhelm absorbs that space into the wide character, so copying or link detection loses
the space between the wide character and the text after it.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/grid_output.rs:252`.

In crates/farhelm-supervisor/src/grid_output.rs:249-257, wide_glyph_owns_next_cell absorbs a non-spacer cell when
next.c == ' ', zerowidth is empty, hyperlink is None, flags minus WRAPLINE are empty, and wire_color(next.bg, colors) ==
style.bg. When a program prints "a界 Z" (wide glyph at cols 1..3, space at col 3, 'Z' at col 4) and deletes one
character at col 2 (CSI 1;3H CSI P), vendor/alacritty_terminal/src/term/mod.rs:2021-2047 (delete_chars) shifts the
printed space from col 3 into col 2 and 'Z' into col 3. Because col 2 is now a plain space matching the wide glyph's
background, wide_glyph_owns_next_cell returns true, emitting '界' with width=2 and advancing col+=2 directly to 'Z'.
Copying, word selection, and printed URL/path detection then see "a界Z" instead of "a界 Z", diverging from
crates/farhelm-terminal-emulator/src/screen.rs:63-90 (which skips only Flags::WIDE_CHAR_SPACER and therefore emits "a界
Z"). The new test deleting_the_wide_spacer_keeps_a_shifted_printed_character
(crates/farhelm-supervisor/src/grid_output/tests.rs:354-375) only checks a non-space shifted character ('Z'). Note the
tradeoff: alacritty_terminal's Cell representation does not distinguish a shifted plain space from an erased right-half
blank of the same background, so resolving this requires either splitting whenever WIDE_CHAR_SPACER is absent (giving up
full-width display on same-bg right-half erase), patching alacritty_terminal to clear the wide head on half-erase/DCH,
or explicitly documenting and accepting this edge case in SPEC.md / SPEC_impl.md.

Reviewer metadata (g27-terminal-wide-links-erase/f1): suggested bucket: other (could not be in highest: loses a single
space character in a rare DCH-on-wide-spacer sequence, not user-owned durable work or security-impacting); confidence:
confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g27-terminal-wide-links-erase/f1`.
