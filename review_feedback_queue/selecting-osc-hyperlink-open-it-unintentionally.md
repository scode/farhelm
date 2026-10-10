# Selecting an OSC 8 hyperlink can open it unintentionally

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Selecting hyperlink text can also open its target.

## Details

F100 — **definite** — `crates/farhelm-ui/assets/terminal.js:3531` — Selecting an OSC 8 hyperlink can open it
unintentionally

Dragging within one program-emitted hyperlink can satisfy the terminal library's activation condition while creating a
text selection. Farhelm's hyperlink callback then opens the URL, so a copy gesture can unexpectedly launch a browser tab
or the system browser. Apply the existing selection guard to this hyperlink activation path while preserving ordinary
click-to-open behavior.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1 activates a matching press/release link without inspecting selection in
  Linkifier._handleMouseUp.
- crates/farhelm-ui/assets/terminal.js:3531 opens OSC 8 targets unconditionally.
- crates/farhelm-ui/assets/terminal.js:3609 explicitly guards the corresponding plain-link path with
  term.getSelection().
- SPEC.md:1271 promises opening on click.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- The confirmation-free click and hover-disclosure decisions do not authorize selection gestures to open links. No
  matching open item found.

Caveats:

- No browser reproduction was performed.
- Both gesture endpoints must remain inside the same link.
- The established consequence is unwanted external navigation; credential disclosure or further exploitation is not
  established.
- Both gesture endpoints must remain within the same link.
- No browser reproduction.
- Do not infer credential disclosure or further exploitation.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_03_cor:p1:F1`.

- `ui_desktop_03_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: high.
