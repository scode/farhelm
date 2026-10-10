# Mouse fidelity assertion accepts UTF-8-corrupted reports

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The mouse-fidelity test accepts the encoding corruption it claims to reject.

## Details

F257 — **definite** — `e2e/tests/mouse-modes.spec.ts:338` — Mouse fidelity assertion accepts UTF-8-corrupted reports

High mouse-coordinate bytes can be expanded as UTF-8 while still satisfying the regression's byte assertions. The test
therefore provides no proof that the browser preserves the intended binary report. Compare complete expected press and
release reports, including exact coordinate bytes and total lengths, so that this specific encoding transformation fails
the oracle.

## Evidence and triage context

- e2e/tests/mouse-modes.spec.ts:113-120 returns exactly three bytes after the prefix, regardless of actual report
  length.
- e2e/tests/mouse-modes.spec.ts:338-343 asserts only three returned bytes and a second byte above 0x7f.
- A body 20 a0 22 expanded to 20 c2 a0 22 yields the accepted slice 20 c2 a0; prefix counting at :95-102 also remains
  unchanged.
- crates/farhelm-fixtures/src/fake_agent.rs:2222-2244 echoes input bytes without report validation.
- crates/farhelm-ui/assets/terminal.js:5345-5347 currently sends the correct byte conversion.
- e2e/tests/mouse-modes.spec.ts:217–224 chooses a column between 97 and 200; :313–314 fixes row 2. UTF-8 encoding
  expands every selected column byte into two bytes, with its leading byte still greater than 0x7f.
- e2e/tests/mouse-modes.spec.ts:113–122 unconditionally extracts exactly three bytes after the prefix. Its sole caller
  at :338–343 checks only length 3 and column byte > 0x7f. For column 200, corrupted body 20 c3 a8 22 becomes extracted
  body 20 c3 a8 and passes both checks.
- e2e/tests/mouse-modes.spec.ts:95–102 counts only the unchanged three-byte prefixes. The checks at :332–337 and
  :367–374 therefore accept the corrupted press/release reports.
- crates/farhelm-fixtures/src/fake_agent.rs:2229–2244 echoes every received byte as hex. At :2364–2377 the cue scanner
  skips exactly three legacy body bytes; the remaining row byte 22 reaches :2352 and clears the word. Subsequent sgr is
  recognized through :2337–2349 and :2444–2460, so the later assertion at e2e/tests/mouse-modes.spec.ts:381–383 does not
  reject this corruption.
- crates/farhelm-ui/assets/terminal.js:5345–5347 currently uses the correct byte conversion. Replacing that call with
  TextEncoder encoding would leave crates/farhelm-ui/assets/term-bytes.js:40–45 intact; its separate unit tests at
  crates/farhelm-ui/js-tests/term-bytes.test.js:27–67 do not exercise this caller. The helm forwards binary frame bytes
  at crates/farhelm-helm/src/terminal.rs:676–677.
- SPEC_impl.md:1131–1134 requires mouse sequences to be forwarded as input; :3583–3589 describes the separate
  byte-for-byte unit contract. Neither accepts an end-to-end oracle that passes corrupted coordinates.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:3583-3588 describes unit coverage of the converter, but does not cover bypassing that converter at the
  browser handler or justify this incorrect browser oracle. No matching queue or ledger disposition found.

Caveats:

- No mutation run performed. The shipped handler is currently correct.
- Source-only verification; no mutation experiment or runtime test was performed.
- The shipped conversion is currently correct. The confirmed defect is in the browser test oracle.
- TODO.md:33–43 Planned covers session creation blocking the connection reader, not this oracle. BUGS.md, the queue
  index and targeted queue/ledger searches supplied no matching trigger, consequence and scope.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_08_sec:p2:F1`,
`test_infrastructure_08_cor:p3:F1`.

- `test_infrastructure_08_sec:p2:F1`: confidence as filed: definite; suggested bucket as filed: other.
- `test_infrastructure_08_cor:p3:F1`: confidence as filed: definite; suggested bucket as filed: other.
