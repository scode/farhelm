# Delete-line counts cause excessive allocation and CPU work

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A delete-line command can keep allocating after all affected rows are blank.

## Details

F35 — **definite** — `crates/farhelm-ui/assets/vendor/xterm.js:1; bytes [173552,174166); R:3897–3905`;
`crates/farhelm-ui/assets/vendor/xterm.js:1, InputHandler.deleteLines, R3897-3904, bytes [173552,174165)` — Delete-line
counts cause excessive allocation and CPU work

The delete-line handler checks that the cursor is in the scrolling region but does not bound the numeric count by the
affected height. It repeatedly removes rows, allocates blanks, and moves buffer entries even after further deletions
have no visible effect. The synchronous work can freeze other window controls; exact resource cost was not measured.
Bound deletion by the remaining scrolling-region height while preserving the upstream bundle's provenance.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1 registers CSI M; deleteLines repeatedly deletes and inserts blank lines
  without clamping to the affected region height.
- crates/farhelm-ui/assets/terminal.js:4337 reaches the synchronous handler.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3511-3512: CSI M dispatches to deleteLines.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3898-3903: cursor validation is followed by an unclamped t-- loop
  performing two splices and a blank-row allocation.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R2995-3011, R6674, R6745 and R6505: each splice performs work; parameter
  saturation remains 2,147,483,647; dispatch does not yield.
- crates/farhelm-ui/assets/terminal.js:4337, :4906-4932 and crates/farhelm-supervisor/src/tmux/query_strip.rs:44-55: the
  shipped integration neither intercepts nor clamps this operation.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- No exact coverage. TRIAGE_OUTCOMES.md:6061 addresses incoming bytes; SPEC.md:2290 does not establish disproportionate
  complexity for bounding this loop.
- SPEC.md:2290-2295 accepts difficult-to-avoid degradation conditionally; a region-height clamp is not shown to require
  elaborate complexity. TRIAGE_OUTCOMES.md:6061-6094 bounds incoming frame size, not this expansion.

Caveats:

- Requires the cursor inside the scrolling region; no exhaustion experiment was performed.
- Requires a cursor in the scrolling region.
- No exhaustion experiment.
- Fixing insertLines alone leaves this independent site.
- Requires the cursor inside the scrolling region.
- Impact is established structurally, without resource benchmarking.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_02_sec:p1:F4`, `vendor_02_cor:p3:F11`.

- `vendor_02_sec:p1:F4`: confidence as filed: definite; suggested bucket as filed: highest.
- `vendor_02_cor:p3:F11`: confidence as filed: definite; suggested bucket as filed: highest.
