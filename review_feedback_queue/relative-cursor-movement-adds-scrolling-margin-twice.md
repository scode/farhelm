# Relative cursor movement adds the scrolling margin twice

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Relative cursor movement draws on the wrong row with a nonzero scrolling margin.

## Details

F111 — **definite** —
`crates/farhelm-ui/assets/vendor/xterm.js:1, InputHandler._moveCursor, R3773-3775, bytes [168778,168881)` — Relative
cursor movement adds the scrolling margin twice

In origin mode, where row positions are relative to the top scrolling margin, the cursor setter expects a relative row
but receives an absolute one. It adds the margin again, so even horizontal movement can unexpectedly shift downward and
make programs overwrite another row. Convert absolute rows to origin-relative rows before calling the setter through an
upstream correction or reviewed integration mitigation.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1, R3770-3774: _setCursor adds scrollTop in origin mode, while _moveCursor
  passes the already absolute activeBuffer.y plus the delta.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3776-3812: vertical and horizontal relative movement commands call
  _moveCursor.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R4015 and R4259-4262: origin mode and a nonzero scrollTop are established
  by supported parser commands. With scrollTop=2 and y=2, a rightward move requests row 2 and the setter stores row 4.
- crates/farhelm-ui/assets/terminal.js:4906-4932: the custom query handlers do not intercept these movement commands.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- Requires origin mode and a nonzero top margin; bottom-margin clamping can conceal some movements.
- No browser execution was performed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_02_cor:p3:F3`.

- `vendor_02_cor:p3:F3`: confidence as filed: definite; suggested bucket as filed: high.
