# Scroll-down counts cause billions of redundant operations

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A scroll-down command has its own excessive-work path.

## Details

F37 — **definite** — `crates/farhelm-ui/assets/vendor/xterm.js:1; bytes [175146,175534); R:3921–3925`;
`crates/farhelm-ui/assets/vendor/xterm.js:1, InputHandler.scrollDown, R3921-3924, bytes [175146,175533)` — Scroll-down
counts cause billions of redundant operations

The downward-scroll handler independently allocates and moves rows for the full accepted count. Once the scrolling
region is blank, further iterations are redundant, yet the GUI thread remains occupied until they finish. Correcting
upward scrolling or the downward handler's background color would leave this availability defect. Clamp the count to
region height through an upstream correction or integration workaround. No browser-stall timing was measured.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1 registers CSI T; scrollDown independently loops over the accepted count
  while allocating and splicing lines.
- crates/farhelm-ui/assets/terminal.js:4337 sends incoming bytes into the synchronous parser.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3517-3518 and R3921-3924: CSI T executes a t-- loop deleting the bottom
  row and inserting a blank at the top for every requested repetition.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R2995-3011: the splices perform real array movement.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R6674, R6745 and R6505: parameters can reach 2,147,483,647 and the handler
  is synchronous.
- crates/farhelm-ui/assets/terminal.js:4337, :4906-4932 and crates/farhelm-supervisor/src/tmux/query_strip.rs:44-55: no
  shipped interception bounds CSI T.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- No exact coverage. The input-byte limit in TRIAGE_OUTCOMES.md:6061 does not constrain this expansion.
- SPEC.md:2290-2295 is not exact coverage: a region-height bound avoids this redundant work. The existing terminal
  frame-budget decision covers a different resource boundary.

Caveats:

- No full-count or browser experiment was run.
- No full-count timing.
- Preserve the separate scroll-down site.
- No browser-stall measurement.
- Same method as F7, with a separate trigger, consequence, and correction.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_02_sec:p1:F6`, `vendor_02_cor:p3:F13`.

- `vendor_02_sec:p1:F6`: confidence as filed: definite; suggested bucket as filed: highest.
- `vendor_02_cor:p3:F13`: confidence as filed: definite; suggested bucket as filed: highest.
