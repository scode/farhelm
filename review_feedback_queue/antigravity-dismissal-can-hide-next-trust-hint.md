# The occurrence-only close can dismiss a later trust question on the same session

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A delayed close from an earlier Antigravity trust hint can hide a later hint in the same session.

## Details

SPEC.md:1741–1742 and :1765–1769 limit × to the shown occurrence. trust_band.rs:213–227 retires stored dismissals when
an observed row ceases asking, but the drawn callback at :472–484 can reinsert the same `(key, agent)` after that
retirement without checking whether its occurrence is current. GPUI dispatch can still use the old rendered hitbox and
listener before redraw (vendor/gpui-pre/src/window.rs:2628–2633,6214–6217,6423–6457.
Vendor/gpui-pre/src/elements/div.rs:3178–3214). If the next relevant observation is the same key and agent asking again,
the stale dismissal passes retention and hides the new hint at trust_band.rs:366. The existing test at
trust_band_tests.rs:547–567 covers × before observing the end, rather than click delivery afterwards. The wire supplies
a sampled boolean, so the UI can bind only to occurrence transitions it actually observes. Create a local identity for
each observed question occurrence and bind × to that identity, preserving the existing lifetime across session selection
and filtering. No subsequent change to the band in range.

Confidence: confirmed by inspection given the documented pre-redraw dispatch and observed false-to-true transition.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `agUI/l11`.
