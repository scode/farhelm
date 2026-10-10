# Erasing through the bottom-right cell throws and interrupts output processing

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An erase command could wedge a connected terminal's output.

## Details

F109 — **possible** —
`crates/farhelm-ui/assets/vendor/xterm.js:1, InputHandler.eraseInDisplay, R3852-3854, bytes [171454,171746)`;
`crates/farhelm-ui/assets/vendor/xterm.js:1; R:3853` — Erasing through the bottom-right cell throws and interrupts
output processing

Erasing to the display's beginning from the bottom-right corner of a fresh normal buffer accesses a nonexistent next
row. The synchronous exception escapes before the write queue advances or schedules continuation, leaving later output
blocked. Browser reproduction was not performed; unrelated controls, host processes, and files are not established as
affected. Obtain an upstream correction or reviewed integration mitigation that uses the buffer offset and checks the
next row's existence.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1, R4907-4915: a fresh buffer contains exactly the viewport rows and has
  ybase=0.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3499-3503 and R3853: CSI 1 J and its selective form invoke
  eraseInDisplay; at the last column of the bottom row, lines.get(y+1) returns no row and .isWrapped throws. The index
  also omits ybase.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3270, R6503-6506 and R6122-6144: parsing runs synchronously inside
  WriteBuffer._action. A throw bypasses the callback, offset advancement, and next scheduling; later writes see the
  nonempty queue and do not restart it.
- crates/farhelm-ui/assets/terminal.js:4334-4343 and :4703-4737: received bytes enter term.write, and pending-byte
  accounting depends on its completion callback.
- R:3499–3503 registers erase-in-display. With ybase=0, y=rows-1 and x=cols-1, R:3853 dereferences
  lines.get(rows).isWrapped. R:6134 calls the handler without a synchronous catch; :6141–6144 advances the queue and
  schedules further work only afterward. terminal.js:4337 supplies the write. No matching acceptance or queue item was
  found. Retain definite throw/possible persistent parser-stall consequences; session-process loss is not established.

Retained confidence: **possible**. Suggested bucket: **high**.

Possible cover:

- SPEC_impl.md:710-719 addresses stale viewport painting, not parser exceptions or stranded writes. FILTER.md:24-42 does
  not establish a rare, self-correcting trigger here.

Caveats:

- No independent browser reproduction was run.
- The proven consequence is the affected terminal's output wedge; unrelated GUI failure, host process loss, and durable
  file loss are not established.
- The undefined-row case is established for a fresh normal buffer; circular-buffer wrapping can change the outcome in
  other states.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_02_cor:p3:F1`, `vendor_02_sec:p1:C1`.

- `vendor_02_cor:p3:F1`: confidence as filed: definite; suggested bucket as filed: high.
- `vendor_02_sec:p1:C1`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
