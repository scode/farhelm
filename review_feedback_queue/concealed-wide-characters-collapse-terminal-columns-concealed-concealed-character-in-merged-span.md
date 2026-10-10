# Concealed wide characters collapse terminal columns — Concealed character in merged span

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A concealed wide character collapses columns inside a merged span.

## Details

F220 — **definite** — `crates/farhelm-ui/assets/vendor/xterm.js:1; module 1433; bytes [85310,85624); readable 1776–1779`
— Concealed wide characters collapse terminal columns — Concealed character in merged span

In the merged-span branch, rendering substitutes one space for a concealed two-column glyph while width compensation
still measures the original glyph. When that measurement equals two cells, compensation is zero and following text
shifts left. Display, cursor, and mouse coordinates disagree. Obtain an upstream correction preserving the cell's
declared width and calculating spacing from the actual replacement representation at this branch.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1; representation [private review artifact] skips zero-width continuation
  cells.
- Readable line 1776 calculates spacing as declared cell width minus the original glyph's measured width; readable line
  1778 appends one space for a concealed glyph in a merged span.
- Readable line 1799 also substitutes one space when creating a new span; readable line 1842 applies the original-glyph
  spacing without a compensating declared span width.
- Readable line 396 selects the DOM renderer used by the shipped integration.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No matching accepted behavior, Planned work, queue item or triage decision found. Valid concealed wide terminal text
  is not an unsupported client input.

Caveats:

- The visible displacement depends on the font's glyph-versus-space measurements.
- No browser pixel measurement was performed.
- No command execution or persistent user-data loss was established.
- No browser or pixel proof was run for this finding.
- Exact displacement depends on font measurements and surrounding span construction.
- No input corruption, execution or persistent work loss was established.
- Preserve both replacement locations in the report; one correction to replacement width accounting can address the
  common cause.
- Respect the unpatched-vendor policy.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_01_cor:p1:F7`.

- `vendor_01_cor:p1:F7`: confidence as filed: definite; suggested bucket as filed: other.
