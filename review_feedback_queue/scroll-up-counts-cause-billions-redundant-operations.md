# Scroll-up counts cause billions of redundant operations

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A scroll-up command can block the window with redundant row operations.

## Details

F36 — **definite** — `crates/farhelm-ui/assets/vendor/xterm.js:1; bytes [174758,175146); R:3916–3920`;
`crates/farhelm-ui/assets/vendor/xterm.js:1, InputHandler.scrollUp, R3916-3919, bytes [174758,175145)` — Scroll-up
counts cause billions of redundant operations

The `CSI S` handler removes and inserts rows once for every requested count, without limiting that count to the
scrolling-region height. It does not turn excess repetitions into meaningful scrollback: after one region height, the
visible effect is complete. Counts can nevertheless reach billions, all before the write scheduler regains control.
Bound effective work by region height through an upstream fix or integration mitigation. Exact freeze duration remains
unmeasured.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1 registers CSI S; scrollUp performs an allocation and two buffer operations
  for every count unit.
- crates/farhelm-ui/assets/vendor/xterm.js:1 accepts a 2147483647 count and checks scheduling time only after the
  handler returns.
- crates/farhelm-ui/assets/terminal.js:4337 supplies the bytes.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3515-3516: CSI S dispatches to scrollUp.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3916-3919: each supplied repetition removes the top row and inserts a
  blank row at scrollBottom, without a count clamp.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R2995-3011: splice performs row movement and mutation notifications.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R6674, R6745, R6505 and R6134-6142: counts reach 2,147,483,647 and the
  write budget is checked only after synchronous parsing returns.
- crates/farhelm-ui/assets/terminal.js:4337 and crates/farhelm-supervisor/src/tmux/query_strip.rs:44-55: CSI S reaches
  this handler.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- No exact coverage; transport caps and the conditional availability acceptance do not dispose of this count-controlled
  expansion.
- SPEC_impl.md:710-719 covers stale scrolled-back painting, not command-count amplification. SPEC.md:2290-2295 supplies
  no exact acceptance for this easily bounded loop.

Caveats:

- Exact freeze duration and renderer behavior were not measured.
- No full-count timing.
- Keep the scroll-up implementation site separate.
- No timing or exhaustion reproduction.
- The finding concerns CSI S, not all upward-scroll mechanisms.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_02_sec:p1:F5`, `vendor_02_cor:p3:F12`.

- `vendor_02_sec:p1:F5`: confidence as filed: definite; suggested bucket as filed: highest.
- `vendor_02_cor:p3:F12`: confidence as filed: definite; suggested bucket as filed: highest.
