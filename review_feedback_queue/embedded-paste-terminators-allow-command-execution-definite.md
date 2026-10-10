# Embedded paste terminators allow command execution — definite

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An embedded paste terminator could make pasted text submit commands.

## Details

F30 — **possible** — `crates/farhelm-ui/assets/vendor/xterm.js:1; module 7861; bytes [8995,9191); R:174–180`;
`crates/farhelm-ui/assets/vendor/xterm.js:1, module 7861, bytes [8949,9190), readable lines 170–179` — Embedded paste
terminators allow command execution — definite

The terminal wraps pasted text in bracketed-paste delimiters but preserves an embedded end delimiter and turns newlines
into carriage returns. A receiving program that honors the embedded terminator can interpret the remaining carriage
returns as ordinary input, potentially submitting commands without a separate Enter gesture. Browser clipboard
preservation and the complete live forwarding path remain unverified. Sanitize framing controls before adding the outer
delimiters through an upstream update or integration workaround, preserving the unmodified vendor bundle.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1, module 7861, wraps pasted text in ESC[200~ and ESC[201~ while preserving
  embedded delimiters and converting newlines to carriage returns.
- crates/farhelm-ui/assets/terminal.js:2441 leaves ordinary text paste to xterm; :4971 forwards emitted data through
  :4943 unchanged when no Shift+Enter merge is armed.
- crates/farhelm-supervisor/src/tmux/input.rs:300 uses send-keys -H for control bytes.
- crates/farhelm-ui/assets/terminal.js:1013 permits OSC 52 system-clipboard writes.
- crates/farhelm-ui/assets/vendor/xterm.js:1; token-verified representation [private review artifact] newline
  normalization and bracket wrapping preserve embedded terminators; clipboard text enters this path at readable
  line 193.
- crates/farhelm-ui/assets/terminal.js:2444 leaves text pastes to xterm; line 4943 encodes outgoing text; line 4971
  forwards onData.
- crates/farhelm-supervisor/src/tmux/input.rs:118 documents byte-preserving control-byte delivery through send-keys -H.
- Actual-token offline proof emitted ESC[200~safeESC[201~evil\rESC[201~ unchanged.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:2171 permits clipboard replacement and intentional paste delivery, but does not explicitly accept embedded
  controls escaping paste framing. TRIAGE_OUTCOMES.md:2290 concerns a different header-copy defect.
- SPEC.md:2171 permits clipboard replacement and user-initiated delivery, but does not explicitly accept pasted data
  escaping bracketed-paste handling. No matching Planned item, BUGS entry, queue item, filter or triage decision found.

Caveats:

- Execution requires a subsequent user paste and a receiving program that interprets bracketed-paste terminators.
- The source report's Bash reproduction was not rerun; browser clipboard preservation and the complete live path were
  not exercised.
- Requires a user paste into a receiver that interprets bracketed paste.
- The producer's isolated Bash reproduction does not establish browser clipboard preservation or the complete live
  Farhelm path.
- Any mitigation must preserve the upstream-identical vendor policy in SPEC_impl.md:718.
- Requires a paste containing an embedded terminator and a receiving program using bracketed paste.
- Browser preservation of the control bytes and complete Farhelm/tmux execution were not independently tested.
- The source report's Bash reproduction was not rerun.
- Browser clipboard preservation and the complete browser-to-shell sequence were not exercised.
- All nine offline source proofs passed in retained run 9c867919-34f1-42d6-b3b6-91c48e8bf5df under [private review
  artifact] Original and representation SHA256 hashes matched the manifest.
- Earlier runs 566a1d46-c404-4bd2-8537-d05df666f082, ffdfb124-67bb-451e-b871-100a27cd7faa and
  4260ec59-f394-45b2-83e0-dbb917e31c6a remain retained; their failures were proof-harness extraction syntax, not product
  failures. Dispositions are recorded in adjudicate_vendor/proof-attempts.txt.
- Any remedy must respect SPEC_impl.md:718: upstream update or integration workaround, not editing vendor bytes.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_01_sec:p1:F1`, `vendor_01_cor:p1:F1`.

- `vendor_01_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
- `vendor_01_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
