# The recovery warning is cut off in a supported narrow terminal

Reviewed commit: 0592c5c6d667abe2f08706185b7566febb3434f7

## TLDR

Scrolling to the recovery warning after a full-screen program exits can reveal only the beginning of its message.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/terminal.rs:1288`.

viewport.rs 61–74,95–100 now allows the trimmed-history notice on an exited alternate frame. Terminal.rs 1114–1128
shapes the full 42-character warning at terminal text size, but 1266–1293 draws it as one line from a four-pixel inset
under the current grid's content mask, with no wrapping or separate full-text disclosure. The pin's monospace advance is
0.6 times the point size (terminal/tests.rs 275–307). At the ordinary 14-point size the text alone therefore needs 352.8
logical pixels. Main_view.rs 215–222 applies the main-pane floor and sidebar/tests.rs 718–757 asserts shipped 320- and
324-pixel main panes at the supported window minimum. Either is already narrower than the notice before the grid's own
padding or scrollbar (attached.rs 296, 1000–1001, 1848–1864). Horizontal shell scrolling reveals the pane, not text
clipped by the painter's own mask. SPEC.md tip 3302–3304 and 3324–3325 requires a scrollable warning naming the
unrecovered output. Existing attached/tests.rs 1374–1385 and 3047–3059 read TerminalView::terminal_text, which directly
returns TRIMMED_TEXT for a Notice (terminal.rs 423–438). Those assertions cannot detect glyph clipping. The painter
predates the assigned commits. The patch exposes its narrow-width limitation on the newly browsable exited alternate
frame. Fit or reflow the app-owned message within the actual visible bounds, with an assertion that checks the rendered
warning rather than only the row content. Bucket other for incomplete notice text, with no claim that the original
recovery loses additional output. Confidence confirmed by pinned renderer and existing layout/font assertions. No visual
or runtime result observed. Possible coverage none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `scrollback/l18`.
