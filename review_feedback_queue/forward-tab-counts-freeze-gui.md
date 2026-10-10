# Forward-tab counts can freeze the GUI

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A tiny forward-tab command can occupy the GUI with billions of redundant steps.

## Details

F32 — **definite** — `crates/farhelm-ui/assets/vendor/xterm.js:1; bytes [170169,170339); R:3821–3826`;
`crates/farhelm-ui/assets/vendor/xterm.js:1, InputHandler.cursorForwardTab, R3821-3825, bytes [170169,170338)` —
Forward-tab counts can freeze the GUI

The terminal accepts a forward-tab count up to 2,147,483,647 and processes it synchronously. Once the cursor reaches the
final column, each further step returns the same position, but the loop continues. Incoming output can therefore block
unrelated window controls long after its visible effect is complete. Exact stall duration was not measured. Stop when
movement saturates or bound traversal by the remaining columns through an upstream update or integration guard.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1 registers CSI I and implements cursorForwardTab with an unclamped
  count-controlled loop; nextStop keeps returning the final column.
- crates/farhelm-ui/assets/vendor/xterm.js:1, module 7262, accepts counts up to 2147483647; the write scheduler checks
  its time budget only after the parsing action returns.
- crates/farhelm-ui/assets/terminal.js:4337 sends received bytes directly to term.write.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3497-3498 and R3821-3825: CSI I dispatches to a loop that decrements the
  entire supplied count.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R5060-5062: nextStop saturates at cols-1, without terminating the caller's
  loop.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R6674 and R6740-6745: numeric parameters can reach 2,147,483,647.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R6503-6506 and R6130-6144: the handler completes synchronously before the
  write queue checks its time budget.
- crates/farhelm-supervisor/src/tmux/query_strip.rs:44-55 does not strip this command; stream.rs:1295-1308 forwards
  ordinary filtered bytes and passthrough bytes. terminal.js:4703-4737 and :4337 feed them to xterm.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:2290 does not accept readily bounded redundant parser work. TRIAGE_OUTCOMES.md:6061 and client.rs:2000 limit
  incoming bytes, not expanded work. No exact queue or Planned coverage.
- SPEC.md:2290-2295 conditionally accepts availability degradation when not easily avoided; stopping at the last column
  is a proportional bound. TRIAGE_OUTCOMES.md:6061-6094 covers oversized frame queues, not CPU expansion inside a small
  frame.

Caveats:

- Full-count timing was not measured.
- Requires a cursor before the wrap-pending position; the default cursor satisfies this.
- Requires the cursor before the wrap-pending position.
- Exact freeze duration was not measured.
- Keep this implementation site separate from backward tab.
- No stall duration was measured.
- The established risk is viewer-thread availability, not host process destruction or credential disclosure.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_02_sec:p1:F1`, `vendor_02_cor:p3:F8`.

- `vendor_02_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
- `vendor_02_cor:p3:F8`: confidence as filed: definite; suggested bucket as filed: highest.
