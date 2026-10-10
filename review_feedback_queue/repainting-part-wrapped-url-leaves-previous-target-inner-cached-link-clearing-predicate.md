# Repainting part of a wrapped URL leaves its previous target clickable — Inner cached-link clearing predicate

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The inner clearing check can preserve a stale target after partial URL repaint.

## Details

F53 — **definite** — `crates/farhelm-ui/assets/vendor/xterm.js:1; module 8906; bytes [39196,39529); readable 770–771` —
Repainting part of a wrapped URL leaves its previous target clickable — Inner cached-link clearing predicate

Even if the repaint callback reaches link clearing, its inner containment check can refuse to clear a cached wrapped URL
after an intersecting partial update. Clicking that retained link can open its previous address rather than the visible
address. No browser navigation was performed, and allowed-web-URL restrictions remain. Correct this separately editable
predicate alongside the outer invalidation check through an upstream fix or integration workaround that preserves vendor
bytes.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1; representation [private review artifact] guards invalidation with complete
  containment; readable line 770 independently repeats that predicate in _clearCurrentLink.
- The actual _clearCurrentLink tokens retained a link spanning rows 2–3 for repaint range 3–3, and cleared it for range
  2–3.
- Readable line 764 activates cached text after matching press/release identities; readable line 3677 reaches
  dirty-row-range refreshes after parsing.
- crates/farhelm-ui/assets/terminal.js:3608 installs the plain-link adapter; its selection and scheme checks do not
  revalidate displayed text.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:1280 explicitly relies on displayed plain URLs identifying their destinations. TRIAGE_OUTCOMES.md:2359
  concerns target disclosure for explicit OSC 8 links, not stale plain-link targets. No applicable coverage found.

Caveats:

- Requires a previously resolved wrapped link and a partial content update.
- Static verification establishes the stale-target path; no browser navigation was performed.
- Navigation remains limited to allowed web URLs and requires a click.
- No browser navigation was performed.
- Requires a previously resolved wrapped link, partial repaint and click before another operation invalidates the cache.
- HTTP(S) navigation still requires a click; arbitrary execution is not established.
- Preserve both separately editable predicates in the report. They form one coupled invalidation defect, not two
  independent consequences.
- Vendor bundle must remain unpatched.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_01_cor:p1:F2`.

- `vendor_01_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: highest.
