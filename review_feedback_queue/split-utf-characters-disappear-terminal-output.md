# Split UTF-8 characters disappear from terminal output

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Valid Unicode output can disappear at chunk boundaries.

## Details

F108 — **definite** —
`crates/farhelm-ui/assets/vendor/xterm.js:1, module 726, bytes [233343,233798), readable lines 5843–5860` — Split UTF-8
characters disappear from terminal output

The terminal's streaming decoder uses continuation payload bits as an indication that a split character is present. A
valid split character with the relevant zero payload can therefore disappear or decode incorrectly even though the
source bytes are valid. Ordinary punctuation becomes unreliable. Use an upstream correction that tracks presence
separately from payload bits, or a compatible streaming-decoder workaround, without editing vendor bytes.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1; representation [private review artifact] tests the masked payload of
  buffered continuation bytes for truthiness; 0x80 consequently appears absent.
- The unchanged decoder class decoded E2 80 94 58 as —X in one chunk, but E2 80 followed by 94 58 as X.
- Readable line 3677 routes non-string writes through this decoder.
- crates/farhelm-ui/assets/terminal.js:4703 creates Uint8Array output; line 4737 calls writeBytes; line 4337 calls
  term.write.
- crates/farhelm-supervisor/src/service/connection.rs:1428 chunks raw bytes without preserving Unicode character
  boundaries.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- The non-UTF-8 filesystem-path limitation does not concern valid Unicode terminal output. No other matching coverage
  found.

Caveats:

- No runtime reproduction was performed during verification.
- The corruption is in the viewer's parsed buffer, not the host's files or running processes.
- A reconnect may reconstruct output differently; repainting the already-corrupt buffer cannot restore the missing
  character.
- Proof executes the real decoder class offline; no complete Farhelm session was launched.
- The corruption is in terminal display state, not host files or running processes.
- Repainting the already-decoded buffer cannot restore the lost character; a reconnect may deliver different chunks.
- Use an upstream decoder correction or compatible streaming-decoder integration workaround, preserving vendor bytes.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_03_cor:p1:F1`.

- `vendor_03_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: high.
