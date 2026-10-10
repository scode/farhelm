# Scrollbar dragging reverses in a very short terminal

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A very short terminal reverses scrollbar dragging.

## Details

F210 — **definite** — `crates/farhelm-ui/assets/vendor/xterm.js:1; module 8245; bytes [363790,363861); R:10046–10048` —
Scrollbar dragging reverses in a very short terminal

The scrollbar's minimum thumb size exceeds a track shorter than 20 pixels. Its inverse movement ratio becomes negative,
making downward dragging request upward scrolling; zero thumb travel also causes division by zero. This affects a
supported one-row terminal geometry. Cap the thumb to available space and handle zero travel explicitly through an
upstream correction; an integration minimum height can mitigate it meanwhile.

## Evidence and triage context

- R:10046–10048 imposes a 20-pixel thumb minimum without capping it to track length.
- R:10093–10096 divides drag displacement by that ratio; R:9571–9576 invokes it during dragging.
- R:1114–1117 supplies canvas height as viewport height, and R:10159 creates the vertical scrollbar with no opposite
  scrollbar.
- F:34–35 permits a one-row terminal.
- crates/farhelm-ui/assets/app.css:6168–6191 permits shrinking to zero minimum height; terminal.js:5402 and :5423–5426
  refit on geometry changes.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- review_feedback_queue/FILTER.md:24–42 does not clearly apply: the trigger is supported short geometry, and the defect
  remains while that geometry remains.

Caveats:

- Requires a very short terminal and available scrollback.
- Visual behavior was not reproduced in a browser; the negative/zero arithmetic and drag caller are confirmed.
- No process or retained-content loss was identified.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_04_cor:p1:F1`.

- `vendor_04_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
