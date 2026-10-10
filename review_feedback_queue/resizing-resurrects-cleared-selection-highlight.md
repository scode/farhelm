# Resizing resurrects a cleared selection highlight

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Resizing brings back a selection highlight that was cleared.

## Details

F219 — **definite** —
`crates/farhelm-ui/assets/vendor/xterm.js:1, module 3955, bytes [80075,80365), readable lines 1622–1624` — Resizing
resurrects a cleared selection highlight

After selection clears, the renderer can return without clearing its cached selection model. A later width change
redraws that old model, restoring a highlight even though selection-dependent actions have no matching selection. This
is an ordinary select-clear-resize sequence. Obtain an upstream fix that updates or clears the cached model before
returning for empty selection.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1; representation [private review artifact] clears displayed selection but
  returns before updating the cached selection model when endpoints are absent.
- Readable line 1610 passes the cached endpoints back into selection rendering on resize.
- Actual-token proof showed zero model updates after empty selection and reuse of the old endpoints on resize.
- Readable line 2464 forwards cleared selection state; readable line 2533 clears actual selection only when the row
  count changes.
- crates/farhelm-ui/assets/terminal.js:5402 and line 5424 refit on window and pane resizing.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- The rare-self-correcting-glitch filter requires a rare trigger as well as a bounded consequence. That premise does not
  hold here. No other coverage found.

Caveats:

- The concrete retained-state path is clearest for a column-only resize after selection clearing has completed.
- The false highlight does not itself restore the terminal's actual selected text or prove a wrong clipboard write.
- The source proof verified retained state and reuse, not browser pixels.
- The false highlight does not restore actual selected text; wrong clipboard contents were not established.
- A later redraw or selection operation can remove the visual artifact.
- Vendor bundle must remain unpatched.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_01_cor:p1:F6`.

- `vendor_01_cor:p1:F6`: confidence as filed: definite; suggested bucket as filed: other.
