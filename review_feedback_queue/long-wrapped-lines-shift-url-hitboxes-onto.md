# Long wrapped lines shift URL hitboxes onto the preceding row

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Long wrapped URLs can make text on the preceding row clickable.

## Details

F54 — **definite** —
`crates/farhelm-ui/assets/vendor/addon-web-links.js:1, module 490, bytes [1549,1881), readable lines 75–83` — Long
wrapped lines shift URL hitboxes onto the preceding row

When collecting a sufficiently long wrapped logical line, the URL provider's budget exit leaves its start-row coordinate
one row before the first collected row. The resulting hitbox can sit above the actual URL, so clicking unrelated text
opens a destination that text does not disclose. Navigation remains HTTP(S) and requires a click; no browser click was
reproduced. Keep the returned start row aligned on budget and buffer-boundary exits through an upstream correction or
integration workaround.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/addon-web-links.js:1; representation [private review artifact] decrements the start
  row before checking the backward text budget; readable line 83 returns that decremented row.
- Readable line 38 uses the returned row to map collected-string matches; readable line 85 performs buffer-coordinate
  mapping.
- Actual-token proof with ordinary 80-column wrapped rows placed a URL on row 40 but returned its hitbox starting on
  row 39. The window returned zero-based start 12 while its first collected row was 13.
- crates/farhelm-ui/assets/terminal.js:3608 installs this provider without coordinate correction.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:1277 accepts an OSC 8 warning-classification limitation for wrapped labels. It does not accept shifted
  plain-link activation coordinates. No matching prior disposition found.

Caveats:

- Requires a sufficiently long wrapped logical line.
- No browser click reproduction was performed.
- This establishes misleading activation coordinates, not arbitrary schemes or automatic execution.
- The proof used synthetic buffer rows while executing the unchanged provider and mapping tokens.
- No browser click was performed.
- Keep scope to misleading hitboxes and HTTP(S) navigation requiring a click.
- Use upstream correction or an integration workaround without patching vendor bytes.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_01_cor:p1:F3`.

- `vendor_01_cor:p1:F3`: confidence as filed: definite; suggested bucket as filed: highest.
