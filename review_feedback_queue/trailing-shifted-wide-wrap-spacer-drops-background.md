# Shifted wide-wrap spacer from CSI P (DCH) drops its nondefault background color

Reviewed commit: d558d4cc259d09581ffa43c33dfdd08d7035d7b6

## TLDR

If a terminal program deletes characters on a colored row that ended in a wrapped wide character, the shifted padding
cell loses its background color and leaves a one-column unpainted gap.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/grid_output.rs:154`.

In `vendor/alacritty_terminal/src/term/mod.rs:2021-2047`, `delete_chars` (`CSI P`) shifts cells left via `row.swap`
without stripping `Flags::LEADING_WIDE_CHAR_SPACER` (the test `shifted_wide_head_stops_before_the_wrap_padding` in
`crates/farhelm-supervisor/src/grid_output/tests.rs:248-262` relies on this). When a colored row ending in a wrapped
wide character (e.g. `\x1b[42mabc界` in 4 columns, or `\x1b[42mab界語` in 5 columns) receives `CSI P` (`\x1b[1;1H\x1b[P`
or `\x1b[1;4H\x1b[P`) without a later `CSI @` shifting it back to the last column, the `LEADING_WIDE_CHAR_SPACER` cell
sits at `col < cols - 1` while `row[Column(cols - 1)]` is reset to `Flags::empty()`. In
`crates/farhelm-supervisor/src/grid_output.rs`, line 154 computes `wide_wrap_padding` only from
`row[Column(cols - 1)].flags` (so it is `false`), lines 163-169 unconditionally skip any cell with
`Flags::LEADING_WIDE_CHAR_SPACER` at any column, and lines 225-238 only emit the nondefault padding run when
`wide_wrap_padding` is `true` for `cols - 1`. The shifted spacer cell is therefore omitted from `runs`, leaving an
unpainted default-background gap in the middle of the colored row (e.g. `[(0, 2, "bc", green), (3, 1, " ", green)]` with
column 2 uncovered). Fix direction: only skip `Flags::LEADING_WIDE_CHAR_SPACER` in the main loop when
`wide_wrap_padding && col + 1 == cols`, letting any shifted spacer at `col < cols - 1` (or at `cols - 1` without
`WRAPLINE`) fall through as an ordinary space cell so its style is preserved.

Reviewer metadata (g26-terminal-wide-proto-and-paint/f1): suggested bucket: other (could not be highest: purely a 1-cell
visual background-color hole on a rare terminal edit sequence; no security or data-loss impact); confidence: confirmed;
possible cover: SPEC.md, "Terminal experience" (line 3268, decided 2026-11-21) notes that terminal fidelity targets
common agent CLIs and shells and need not match xterm in rare wide-character edge cases, though it does not explicitly
name DCH-shifted wide-wrap spacers and this same PR series explicitly handled DCH-shifted wide-wrap spacers in
shifted_wide_head_stops_before_the_wrap_padding..

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g26-terminal-wide-proto-and-paint/f1`.
