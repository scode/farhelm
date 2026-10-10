# Repeat-character commands expand tiny input into enormous allocations

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A short repeat-character command requests an enormous allocation.

## Details

F38 — **definite** — `crates/farhelm-ui/assets/vendor/xterm.js:1; bytes [177874,178327); R:3967–3983`;
`crates/farhelm-ui/assets/vendor/xterm.js:1, InputHandler.repeatPrecedingCharacter, R3967-3983, bytes [177873,178327)` —
Repeat-character commands expand tiny input into enormous allocations

After a printable character, the terminal can accept a repetition count of 2,147,483,647 and allocate about 8 GiB even
for one ASCII character. Expansion and printing then run synchronously if allocation succeeds. Allocation refusal, a
stranded parser queue, or expensive copying depend on the engine; no crash or lost work was demonstrated. Bound
expansion before allocating and process supported repetition in bounded batches. Preserve vendor provenance, and assess
semantics carefully because repetition beyond one screen can be meaningful.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1 registers CSI b; repeatPrecedingCharacter allocates
  Uint32Array(previousString.length * count), copies repetitions, and prints the expansion synchronously.
- crates/farhelm-ui/assets/vendor/xterm.js:1 accepts 2147483647, yielding an approximately 8 GiB requested allocation
  for a one-code-unit preceding character.
- crates/farhelm-ui/assets/vendor/xterm.js:1 advances its write queue only after the parsing action returns; there is no
  synchronous exception recovery around that action.
- crates/farhelm-ui/assets/terminal.js:4337 exposes this handler to incoming terminal bytes.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3527-3528: CSI b dispatches to repeatPrecedingCharacter.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3967-3982: after a preceding printable character, the handler allocates
  Uint32Array(previousString.length*count), expands it in a count-driven loop, and prints it synchronously.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R6674 and R6745: counts reach 2,147,483,647; even one UTF-16 code unit
  requests about 8 GiB of typed-array storage at that count.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R6503-6507 and R6130-6144: there is no synchronous exception recovery or
  scheduling boundary inside the operation.
- crates/farhelm-ui/assets/terminal.js:4334-4337 and crates/farhelm-supervisor/src/tmux/query_strip.rs:44-55: byte-based
  flow control does not limit repetition expansion.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- No exact coverage. Transport-frame limits do not bound expanded output. SPEC.md:2290 does not establish that avoiding
  this allocation requires elaborate complexity. All seven vendor handlers remain separate editable locations even if
  remediation shares an integration layer.
- SPEC.md:2290-2295 permits difficult-to-avoid denial-of-service effects, and SPEC_impl.md:718-719 prohibits local
  vendor patches. Neither establishes that an upstream fix or bounded consumer mitigation is impractical.
  TRIAGE_OUTCOMES.md:6061-6094 bounds frame bytes, not expanded output.

Caveats:

- Requires a preceding printable character.
- Depending on the engine, allocation may throw, fail, or succeed before costly copying; no OOM experiment was run.
- No OOM experiment; do not claim a demonstrated process crash or lost session work.
- Preserve upstream vendor provenance when choosing a remedy.
- Allocation refusal, memory pressure, and browser termination depend on engine and machine limits; none was exercised.
- Unlike tabulation or region replacement, repetition can have meaningful output beyond one screen. A simple
  screen-height clamp may change semantics.
- The concrete resource defect is definite; the proportionate remediation needs evaluation.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_02_sec:p1:F7`, `vendor_02_cor:p3:F14`.

- `vendor_02_sec:p1:F7`: confidence as filed: definite; suggested bucket as filed: highest.
- `vendor_02_cor:p3:F14`: confidence as filed: definite; suggested bucket as filed: highest.
