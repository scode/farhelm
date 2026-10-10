# Narrowing the terminal corrupts combining-character text

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Narrowing the terminal changes combining-character text.

## Details

F110 — **definite** —
`crates/farhelm-ui/assets/vendor/xterm.js:1, BufferLine.copyCellsFrom, R5244-5260, bytes [219055,219540)` — Narrowing
the terminal corrupts combining-character text

During narrowing reflow, combined-string copying uses overlapping source and destination regions on the same row. The
copy ignores overlap direction and the upper source boundary, allowing distinct strings to become repeated earlier
strings. Users can read or copy text that differs from the program's output. Use an upstream correction or reviewed
mitigation that snapshots or directionally copies only entries inside the requested interval, preserving vendor
provenance.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1, R5244-5258: numeric cell storage copies backward when requested, but
  Object.keys(_combined) is traversed forward and each string is fetched after earlier destination writes. Only
  r>=sourceStart is checked.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R4968-5008: narrowing wrapped text calls copyCellsFrom with reverse=true;
  source and destination can be the same line. Reflowing two full ten-column rows into nine columns moves the remaining
  eight cells of the second source row from columns 0..7 to 1..8, producing the dangerous overlap.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R5261-5270: translated text reads the altered _combined strings, so the
  defect affects copied text as well as rendering.
- crates/farhelm-ui/assets/terminal.js:5402-5424: window and pane resizing invoke the fit addon;
  crates/farhelm-ui/assets/vendor/addon-fit.js:1, readable lines 15-19, calls terminal.resize.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- SPEC_impl.md:710-719 covers stale painting, not altered cell contents. SPEC.md:1185-1200 defines retention and replay,
  without accepting corruption during resize.

Caveats:

- The source trace establishes browser-buffer corruption, not changes to host files or tmux's retained history.
- Reflow can skip the cursor's wrapped group when reflowCursorLine is disabled; other wrapped groups remain affected.
- No runtime reproduction was repeated.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_02_cor:p3:F2`.

- `vendor_02_cor:p3:F2`: confidence as filed: definite; suggested bucket as filed: high.
