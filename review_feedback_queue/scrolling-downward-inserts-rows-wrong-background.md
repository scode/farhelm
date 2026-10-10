# Scrolling downward inserts rows with the wrong background

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Downward scrolling inserts blank rows with the default background.

## Details

F255 — **definite** —
`crates/farhelm-ui/assets/vendor/xterm.js:1, InputHandler.scrollDown, R3921-3924, bytes [175146,175533)` — Scrolling
downward inserts rows with the wrong background

The downward-scroll handler creates blanks from default attributes, while equivalent operations preserve the current
erase background. A colored terminal application therefore gets incorrectly colored rows when scrolling down. Use
current erase attributes for these rows through an upstream correction or reviewed integration mitigation, separately
from the handler's excessive-count defect.

## Evidence and triage context

- crates/farhelm-ui/assets/vendor/xterm.js:1, R3517-3518: CSI T invokes scrollDown.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3923: newly exposed rows are constructed with DEFAULT_ATTR_DATA.
- crates/farhelm-ui/assets/vendor/xterm.js:1, R3918 and R4424: upward scrolling and reverse index instead call
  getBlankLine(_eraseAttrData()).
- crates/farhelm-ui/assets/vendor/xterm.js:1, R4211 and R4434-4435: background-selection commands update current
  attributes, and erase attributes retain those background color bits.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- SPEC.md:1164 requires color fidelity. FILTER.md:24-34 does not establish that colored downward scrolling is a rare
  qualifying trigger.

Caveats:

- Stored background mismatch is established; pixels were not independently exercised.
- F13 concerns resource bounds in the same method and remains a separate finding.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_02_cor:p3:F7`.

- `vendor_02_cor:p3:F7`: confidence as filed: definite; suggested bucket as filed: other.
