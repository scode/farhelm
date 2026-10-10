# Recursive selection across a very long wrapped word

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Selecting a very long wrapped word could exhaust the browser's stack.

## Details

F301 — **possible** — `crates/farhelm-ui/assets/vendor/xterm.js:1, SelectionService._getWordAt, R2734-2790`;
`crates/farhelm-ui/assets/vendor/xterm.js:1; R:2735–2792` — Recursive selection across a very long wrapped word

Word selection recursively walks adjacent wrapped rows without a depth bound. Remote output can fill thousands of
retained rows with one wrapped word, making an ordinary double-click traverse that recursion. Supported-browser stack
limits and effects beyond failed selection were not measured. Replace recursive traversal with bounded iterative work
through an upstream or integration correction, without claiming a demonstrated persistent window wedge or lost work.

## Evidence and triage context

- R2613-2617 routes word selection into _selectWordAt; R2793-2797 invokes _getWordAt; R2770-2784 recursively traverses
  preceding and following wrapped rows without a depth bound or yield. terminal.js:3464 retains 12,000 scrollback rows,
  allowing thousands of recursive calls. SPEC.md:2268-2271 protects ordinary GUI controls; :2290-2295 only conditionally
  accepts difficult-to-avoid degradation. No exact filter or accepted decision covers this recursion. Stack exhaustion,
  duration, and impact beyond a failed selection remain unverified; no persistent GUI wedge or work loss is claimed.
- R:2646 and :2664–2665 route double-click to word selection; :2616–2617 calls _selectWordAt; :2794 calls _getWordAt.
  R:2773 and :2783 recursively visit adjacent wrapped rows without a depth bound. terminal.js:3464 permits 12000
  scrollback rows. Engine stack limits were not measured, so retain as possible. No exact coverage basis was found.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_02_cor:p3:C8`, `vendor_02_sec:p1:C2`.

- `vendor_02_cor:p3:C8`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
- `vendor_02_sec:p1:C2`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
