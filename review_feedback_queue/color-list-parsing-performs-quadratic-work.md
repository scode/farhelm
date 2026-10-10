# Color-list parsing performs quadratic work

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A long color list could block the GUI with costly array operations.

## Details

F39 — **possible** — `crates/farhelm-ui/assets/vendor/xterm.js:1; bytes [190058,190356); R:4297–4321` — Color-list
parsing performs quadratic work

The color-list handler splits the complete payload, then removes two array heads for every pair, including invalid
pairs, before returning control to the GUI. Reported Node measurements support expensive behavior, but JavaScript does
not guarantee the complexity of these operations and Chromium/WebKit costs remain unmeasured. Traverse the split array
by index instead, and obtain browser evidence when choosing the mitigation. Keep the practical slowdown claim
conditional on engine behavior.

## Evidence and triage context

- R:4299–4303 splits the complete payload and calls shift twice for every pair, including invalid pairs.
- R:6196 sets a 10000000-character payload limit; R:6656–6666 accumulates OSC data across writes until termination.
- crates/farhelm-ui/assets/terminal.js:4337 writes each received chunk into the same parser.
- R:6134–6142 cannot yield while the color handler executes.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:2290–2295 does not clearly cover an array traversal replaceable with indexed iteration.
- TRIAGE_OUTCOMES.md:6061–6094 limits individual transport frames; an OSC payload can span many accepted frames.

Caveats:

- JavaScript does not specify shift implementation complexity.
- The report's Node timings were not rerun and do not establish Chromium/WebKit timings.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_02_sec:p1:F8`.

- `vendor_02_sec:p1:F8`: confidence as filed: definite; suggested bucket as filed: highest.
