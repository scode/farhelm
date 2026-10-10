# Composition fallback can resend previously typed input

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Composition fallback could resend retained terminal input.

## Details

F107 — **possible** —
`crates/farhelm-ui/assets/vendor/xterm.js:1, module 3618, bytes [67221,67594), readable lines 1416–1425` — Composition
fallback can resend previously typed input

The composition fallback calculates an incorrect input difference after an in-place textarea edit and can send
previously typed text again instead of only the new edit. The calculation and forwarding path are established, but a
native event sequence reaching it in the supported application remains unverified. Carry the finding into the existing
investigation and seek an upstream or integration correction. Disabling predictive text does not repair retained-text
replay, and destructive execution is not demonstrated.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1; representation [private review artifact] invokes the fallback for keyCode
  229; readable line 1416 uses string replacement and emits the entire changed value for equal-length edits.
- Actual fallback tokens emitted the entire retained SPÉC after the fixture changed SPEC to SPÉC, rather than only the
  changed input.
- crates/farhelm-ui/assets/terminal.js:4971 forwards onData.
- docs/codex-input-investigation.md:136 and line 185 describe the same retained-input mechanism and distinguish it from
  predictive-text mitigation.
- TODO.md:227 keeps the input-corruption investigation open under Tricky bugs.

Retained confidence: **possible**. Suggested bucket: **high**.

Possible cover:

- No queue item or TRIAGE_OUTCOMES decision was found for this mechanism. Reference the existing investigation rather
  than presenting this as newly discovered. TODO's Tricky bugs entry does not satisfy the requested Planned exclusion.

Caveats:

- No native event sequence was reproduced.
- The findings document explicitly leaves the cause of individual incidents uncertain.
- Prompt/command corruption is supported; destructive execution or loss of work was not independently demonstrated.
- The proof injected a textarea edit and scheduler callback; it did not reproduce native IME or WebKit events.
- The cause of individual reported incidents remains uncertain.
- Prompt or command corruption is the supported consequence; destructive execution and work loss are not demonstrated.
- Do not claim writingsuggestions=false repairs this fallback.
- Seek an upstream or integration correction without modifying vendor bytes.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_01_cor:p1:F8`.

- `vendor_01_cor:p1:F8`: confidence as filed: possible; suggested bucket as filed: high.
