# The constructor-font regression observes a mutable value after construction

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The constructor-font test observes a value that can already have been corrected.

## Details

F239 — **definite** — `e2e/tests/terminal-font.spec.ts:262–273` — The constructor-font regression observes a mutable
value after construction

A terminal can be constructed with the wrong fallback font, then receive the existing post-construction correction
before socket observation. Its mutable current font satisfies the test even though the constructor-time requirement was
violated. Record the initial constructor font option immutably and associate that observation with the tested session.

## Evidence and triage context

- e2e/tests/terminal-font.spec.ts:89–93 waits for a matching open socket.
- e2e/tests/terminal-font.spec.ts:262–273 then reads the terminal's current fontFamily.
- e2e/tests/helpers/terminal-readiness.ts:314–325 observes mounted/socket-open state, not constructor options.
- crates/farhelm-ui/assets/terminal.js:3434 and 3499 choose constructor options from fontReady.
- crates/farhelm-ui/assets/terminal.js:3687–3702 later mutates fontFamily when the font finishes loading.
- crates/farhelm-ui/assets/terminal.js:3438–3447 already provides a constructor-time observation seam.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- TODO.md:24–31 plans removal of fallback behavior under Definite simplification, not Planned.
  TRIAGE_OUTCOMES.md:5977–5991 disposes of retained font-promise memory, not this test's mutable observation. Neither
  exactly covers the finding.

Caveats:

- A concrete false-pass oracle, not evidence that today's common-case constructor chooses the wrong font.
- No runtime reproduction.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `test_infrastructure_12_cor:p1:F2`.

- `test_infrastructure_12_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
