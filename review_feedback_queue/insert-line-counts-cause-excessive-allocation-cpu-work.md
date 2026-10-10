# Insert-line counts cause excessive allocation and CPU work

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An insert-line command can request billions of redundant allocations.

## Details

F34 — **definite** — `crates/farhelm-ui/assets/vendor/xterm.js:1; bytes [172939,173552); R:3887–3896`;
`crates/farhelm-ui/assets/vendor/xterm.js:1, InputHandler.insertLines, R3887-3895, bytes [172939,173551)` — Insert-line
counts cause excessive allocation and CPU work

With the cursor inside the scrolling region, the terminal repeats line insertion for the entire supplied count. It
continues allocating blank lines and moving buffer entries after the region's visible result has saturated, and the
write scheduler cannot yield inside the handler. Untrusted output can consequently monopolize the window thread.
Resource exhaustion was not measured. Clamp the count to the remaining region height through an upstream correction or
suitable integration guard.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1 registers CSI L; insertLines checks cursor eligibility but loops over the
  entire accepted count, allocating blank lines and splicing the buffer.
- crates/farhelm-ui/assets/terminal.js:4337 feeds that handler; the vendor write scheduler cannot yield inside it.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3509-3510: CSI L dispatches to insertLines.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3888-3894: t is the unbounded parameter; the only early rejection
  concerns cursor position. Each repetition removes and inserts a row and creates a blank line.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R2995-3011: CircularList.splice performs actual array movement and emits
  mutations; it does not coalesce excessive counts.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R6674, R6745 and R6505: the count can reach 2,147,483,647 and dispatch is
  synchronous.
- crates/farhelm-ui/assets/terminal.js:4337 and crates/farhelm-supervisor/src/tmux/query_strip.rs:44-55: this command
  reaches xterm without a count limit.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- No exact Planned, BUGS, queue or ledger coverage. Transport caps do not bound parameter expansion; the availability
  exception is not an established acceptance of this local count-bound omission.
- SPEC.md:2290-2295 does not establish acceptance of easily bounded redundant work. Existing frame-budget and clipboard
  ledger decisions concern different mechanisms.

Caveats:

- Cursor must be inside the scrolling region, as it is initially; exhaustion was not attempted.
- Cursor must be within the scrolling region.
- No exhaustion experiment.
- Preserve this independently editable insertion handler.
- The cursor must be inside the scrolling region.
- No measured CPU, allocation, or browser-termination result is claimed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_02_sec:p1:F3`, `vendor_02_cor:p3:F10`.

- `vendor_02_sec:p1:F3`: confidence as filed: definite; suggested bucket as filed: highest.
- `vendor_02_cor:p3:F10`: confidence as filed: definite; suggested bucket as filed: highest.
