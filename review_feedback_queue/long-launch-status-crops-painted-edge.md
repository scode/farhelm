# Long launch-status pills crop their painted edge

Reviewed commit: 372c7ef7aa193093b9c918b5f478cc52dc111441

## TLDR

A long session name, or a valid long host alias in a narrow window, pushes the pending-launch pill's right border and
corner outside the window. Both checked engines show the cosmetic clipping. Elapsed text stays visible; no lost status
information is established.

## Details

Suggested bucket: **other**. Confidence: **confirmed source-matched static CSS geometry** in Chromium and WebKit.
Proposed drop: none. The earlier elapsed-text-clipping hypothesis was refuted by measurement.

`crates/farhelm-ui/assets/app.css:6165–6171` centers and caps the outer `.launch-progress-pill`. `6173–6184` caps its
painted child's content box without including 28px of padding and 2px of border. With overflowing labels, the painted
edge extends 30px beyond the outer box and 10px beyond the viewport. Cap and center the painted border box, including
that padding and border.

The static fixture copied immutable production CSS/fonts and the rendered structure from
`list/create_form.rs:6164–6168`, `launch_progress.rs:77–96,124–134` and `peer.rs:319–337`. It checked ordinary names, a
valid 247-character session title, 64-character maximum host aliases, 1280×800 and 375×667 viewports, web/Mac CSS
classes, and elapsed values 2 and 59. Loaded fonts, exact text, hidden form, content-box sizing and label overflow were
asserted before measuring. No global sizing reset was added.

Each engine completed the corrected 32-case matrix. Twenty long-content cases put the painted right edge at 1290 or
385px while elapsed elements ended at 1275 or 370px, inside their respective viewports. The other twelve cases fit. The
parent inspected the fixture, production structure, compact measurements and reports. This establishes static layout,
not compiled Dioxus clocks/IPC/isolation, native macOS behavior or accessibility.

Both recorder attempts are retained: `a161612c-70a3-497b-8ab4-0d10e70029bf` and `3a8373d5-f949-4522-b97b-500d376eaed1`.
Both completed with exit 1 at the final painted-bounds assertion, after collecting the measurements, with complete
output and no skips. The first included invalid 245-character host aliases; its long-title/ordinary-host cases were
valid. The second corrected aliases to 64 characters and separately completed both valid matrices, but also accidentally
discovered the archived first fixture. Its report has four tests, not two; archived invalid-host repeats receive no
product-evidence credit. No third run was needed. These are retained defect observations, not passing validation of
product layout.

SPEC.md, "Creation", 1005–1010 describes the bottom pending status. No exact acceptance, planned repair, known bug,
queue item or ledger decision covers this sizing defect. FILTER.md does not clearly cover decorative clipping at both
ordinary and narrow widths; no explanation or action is alleged unreachable. The CSS route remains through
`c5ae8548439cc5bfa33c11ef7ce8da6129d8072a`.

Provenance: trailing-review run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, `g056/g056-l8`, event 20. Original concerns,
fixture corrections and the narrower confirmed consequence retain that ID. No application stack, build, server, network
access or live-install operation ran.
