# The alternate horizontal-positioning command also changes the row

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The alternate column-positioning command independently moves the cursor's row.

## Details

F113 — **definite** —
`crates/farhelm-ui/assets/vendor/xterm.js:1, InputHandler.charPosAbsolute, R3802-3804, bytes [169681,169767)` — The
alternate horizontal-positioning command also changes the row

The alternate horizontal-positioning handler passes an absolute row to a setter using origin mode, where rows are
relative to the scrolling margin. A nonzero top margin is added again, so changing only the column shifts the row and
subsequent drawing. Correct this separate call site through an upstream update or reviewed integration mitigation that
preserves the row.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1, R3523-3524: CSI final backtick dispatches to charPosAbsolute.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3803: charPosAbsolute passes activeBuffer.y to _setCursor.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3770-3771: the setter adds scrollTop again under origin mode.
- crates/farhelm-ui/assets/terminal.js:4906-4932: no custom handler replaces this operation.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Requires origin mode with a nonzero top margin.
- No runtime reproduction was repeated.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_02_cor:p3:F5`.

- `vendor_02_cor:p3:F5`: confidence as filed: definite; suggested bucket as filed: high.
