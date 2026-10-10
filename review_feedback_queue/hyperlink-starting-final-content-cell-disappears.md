# A hyperlink starting at the final content cell disappears

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A hyperlink beginning in the row's final content cell disappears.

## Details

F218 — **definite** —
`crates/farhelm-ui/assets/vendor/xterm.js:1, module 3285, bytes [43688,43855), readable lines 895–902` — A hyperlink
starting at the final content cell disappears

The hyperlink scanner emits a link when it reaches a later boundary but does not flush a newly encountered link at the
end of scanning. A valid one-cell label in the final content cell is therefore never offered for hovering or activation.
Obtain an upstream correction that flushes an outstanding link at scan completion; this need not be the physical last
column.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1; representation [private review artifact] scans only trimmed content;
  readable line 895 starts a link and unconditionally continues.
- Readable line 902 contains emission logic; readable line 932 invokes the callback without flushing the outstanding
  final link.
- Actual provideLinks tokens returned zero links for a row consisting of one content cell carrying a valid HTTP(S) OSC 8
  link.
- crates/farhelm-ui/assets/terminal.js:3530 installs the shipped OSC 8 handlers.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC.md:1271 promises terminal-link activation. No matching Planned item, acceptance, queue entry or filter found.

Caveats:

- The triggering cell is the last content cell, not necessarily the terminal's final column.
- Not every single-character link is affected.
- Not every one-character link is affected; the linked cell must be the final content cell.
- Proof executes the real provider with a minimal buffer fixture, not browser input.
- Upstream correction or an integration workaround must preserve vendor provenance.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_01_cor:p1:F5`.

- `vendor_01_cor:p1:F5`: confidence as filed: definite; suggested bucket as filed: other.
