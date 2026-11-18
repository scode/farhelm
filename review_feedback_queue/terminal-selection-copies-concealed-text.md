# Copying terminal text can include content you cannot see

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

Selecting a visible line in the terminal can copy hidden text along with it. Pasting it into another shell or
application can use more than you selected by sight.

## Details

Bucket: **highest**. GUI migration final whole-tree review, R5 Opus 2.

Source references: `crates/gui/farhelm-grid-model/src/selection.rs:293`,
`crates/gui/farhelm-gui/src/terminal.rs:1191, 1322`, `crates/gui/farhelm-gui/src/terminal/attached.rs:4007–4020`.

The terminal paints concealed cells without drawing their text, while selection extraction reads the underlying
characters and writes the result to the system clipboard. A hidden suffix can travel with an otherwise ordinary-looking
command. A later paste elsewhere may act on the whole value.

Opus rated this likely and noncritical, matching the old xterm.js app and common terminal behavior. There is no separate
verification verdict or runtime observation. Highest reflects the source route from remote output through a local copy
gesture to a materially different clipboard value with possible destructive use. SPEC.md “Security” explicitly accepts
remote OSC 52 clipboard writes, including a malicious program replacing the clipboard. That is possible cover for the
consequence. Its stated trigger is an intentional control sequence that writes the clipboard, while this route is
through Farhelm's visible selection.

Omit concealed cells from selection text or disclose them when copying, as the details bar does for hidden characters.
Check how omission preserves line boundaries and prevents joining visible fragments into a different command.
