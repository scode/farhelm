# Repainting part of a wrapped URL leaves its previous target clickable — Outer repaint invalidation predicate

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The outer repaint check lets a changed wrapped URL keep its old target.

## Details

F52 — **definite** — `crates/farhelm-ui/assets/vendor/xterm.js:1; module 8906; bytes [40637,41112); readable 795–802` —
Repainting part of a wrapped URL leaves its previous target clickable — Outer repaint invalidation predicate

After a wrapped URL has been resolved, a repaint affecting only part of it does not satisfy the outer invalidation
condition. The cached link can remain active, so a click opens the previous address rather than the displayed one.
Activation still requires a click and an allowed web URL. Obtain an upstream fix or integration workaround that
invalidates on any intersecting change; the inner clearing condition must also be corrected.

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
