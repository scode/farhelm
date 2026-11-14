# Terminal renderer fixtures

Grid protocol recordings for the terminal renderer's tests (`src/terminal/tests.rs`), in the protocol's JSON
description. Tests encode every message through the real binary encoder and decode it again before the grid model sees
it, so a recording cannot bypass the protocol's width, style and ordering checks; `replay_complete` is a separate
control the tests deliver after the recording.

Both come from the GPUI prototype's terminal (`crates/light/tests/fixtures/` in the maintainer's prototype repository),
unchanged; they still decode with this repository's protocol.

- `attach.jsonl` is the old web client's grid-client recording (`crates/farhelm-ui/js-tests/fixtures/grid/attach.jsonl`
  at `0bc984b`): a sparse screen, a soft wrap, a history append and a resolved OSC 8 link.
- `matrix.jsonl` was written for the prototype: every palette index, bold 0-15, truecolor, OSC 10/11 default colors,
  inverse, dim, italic, hidden and strikethrough, the five colored underlines, one- and two-cell CJK and emoji,
  variation sequences, combining marks, ZWJ emoji, a Nerd Font glyph, bidi control removal, box drawing, blocks and
  ASCII ligature candidates. The tests use it for geometry (wire widths and visibility), not pixels: the screenshot
  goldens draw only ASCII and box drawing, since no fallback font is bundled for goldens yet.
