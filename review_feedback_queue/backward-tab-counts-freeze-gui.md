# Backward-tab counts can freeze the GUI

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A backward-tab command can keep the GUI busy after the cursor reaches zero.

## Details

F33 — **definite** — `crates/farhelm-ui/assets/vendor/xterm.js:1; bytes [170339,170510); R:3827–3832`;
`crates/farhelm-ui/assets/vendor/xterm.js:1, InputHandler.cursorBackwardTab, R3827-3831, bytes [170339,170509)` —
Backward-tab counts can freeze the GUI

The backward-tab handler independently loops over the full accepted count, up to 2,147,483,647. Its position calculation
stops at column zero, but that does not terminate the loop or yield the GUI thread. Remote output can thus request
billions of redundant steps. No full-count timing was performed. Bound traversal by the starting column or stop at zero,
using an upstream correction or integration remedy that preserves vendor provenance.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1 registers CSI Z and implements cursorBackwardTab with the full parameter
  count; prevStop clamps at zero without terminating the outer loop.
- crates/farhelm-ui/assets/terminal.js:4337 reaches that synchronous parser.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3521-3522 and R3827-3831: CSI Z invokes the full-count loop.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R5056-5058: prevStop returns zero once the beginning is reached; it does
  not terminate the caller's repetitions.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R6674, R6740-6745, R6503-6506 and R6130-6144: accepted counts reach
  2,147,483,647 and the handler has no internal yield.
- crates/farhelm-supervisor/src/tmux/query_strip.rs:44-55 and crates/farhelm-ui/assets/terminal.js:4337, :4906-4932:
  neither output filter replaces or bounds CSI Z.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- No exact coverage. The frame-byte mitigation in TRIAGE_OUTCOMES.md:6061 does not bound this count; SPEC.md:2290 is
  conditional on avoidance complexity.
- SPEC.md:2290-2295 is conditional on avoidance difficulty; stopping at column zero is a small semantic bound.
  TRIAGE_OUTCOMES.md:6061-6094 concerns queued frame bytes, not this operation.

Caveats:

- No full-count execution or browser timing was performed.
- No timing experiment.
- Separate editable handler from forward tab; do not collapse away this site.
- No resource-exhaustion probe was run.
- Actual stall duration is engine-dependent.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_02_sec:p1:F2`, `vendor_02_cor:p3:F9`.

- `vendor_02_sec:p1:F2`: confidence as filed: definite; suggested bucket as filed: highest.
- `vendor_02_cor:p3:F9`: confidence as filed: definite; suggested bucket as filed: highest.
