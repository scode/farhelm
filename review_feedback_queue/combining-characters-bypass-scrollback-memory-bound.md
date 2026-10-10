# Combining characters bypass the scrollback memory bound

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Combining characters can grow a single terminal cell beyond line-retention bounds.

## Details

F41 — **definite** — `crates/farhelm-ui/assets/vendor/xterm.js:1; bytes [215636,215920); R:5152–5155` — Combining
characters bypass the scrollback memory bound

Repeated combining characters extend the string stored in one cell without materially moving the cursor or increasing
the retained line count. Completed-write accounting and bounded scrollback therefore do not bound this retained text.
Engine string limits eventually fail rather than provide a safe retention policy; actual exhaustion was not measured.
Define a finite per-cell combined-string limit and an explicit overflow policy through an upstream or integration change
that preserves vendor provenance.

## Evidence and triage context

- R:3700–3719 retains preceding join state and appends joining code points to the previous cell.
- R:5154 concatenates to _combined[e] without a length bound.
- R:3723 preserves join state for subsequent writes.
- crates/farhelm-ui/assets/terminal.js:3464 limits lines to 12000; :4343 subtracts completed input bytes without
  accounting for retained cell strings.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:2290–2295 does not explicitly accept unbounded individual cells.
- SPEC_impl.md:838–841 defines line retention and replay equivalence, not unlimited bytes per cell.
- TRIAGE_OUTCOMES.md:6061–6094 covers frame queues rather than terminal cell storage.

Caveats:

- Engine string limits eventually terminate growth through failure; they are not a safe retained-memory policy.
- Actual memory exhaustion and rendering costs were not measured.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_02_sec:p1:F10`.

- `vendor_02_sec:p1:F10`: confidence as filed: definite; suggested bucket as filed: highest.
