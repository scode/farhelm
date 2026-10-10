# Column positioning unexpectedly changes the row

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Setting the cursor column also changes its row.

## Details

F112 — **definite** —
`crates/farhelm-ui/assets/vendor/xterm.js:1, InputHandler.cursorCharAbsolute, R3796-3798, bytes [169495,169584)` —
Column positioning unexpectedly changes the row

This column-positioning handler preserves the current absolute row when calling a setter that interprets it relative to
the scrolling margin. With that origin mode and a nonzero top margin, the margin is added again. Output lands on the
wrong row. Preserve the absolute row correctly through an upstream correction or reviewed mitigation; correcting
relative movement alone does not cover this handler.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1, R3493-3494: CSI G dispatches to cursorCharAbsolute.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3797: the handler supplies activeBuffer.y unchanged to _setCursor.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3770-3771: _setCursor adds scrollTop when origin mode is enabled,
  producing an unintended row displacement.
- crates/farhelm-ui/assets/terminal.js:4337 and :4906-4932: output reaches the parser without a replacement handler for
  CSI G.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Requires origin mode with a nonzero top margin.
- Shares the coordinate-contract error with F3 and F5 but is a separate editable handler.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_02_cor:p3:F4`.

- `vendor_02_cor:p3:F4`: confidence as filed: definite; suggested bucket as filed: high.
