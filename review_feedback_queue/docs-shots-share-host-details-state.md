# The Manage hosts Details shot changes later host-menu previews on the same Boat member

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A docs screenshot can show different host row details depending on which other shots run on the same worker.

## Details

screenshot and the Start a session host-menu screenshot on one Boat member makes the second image show an expanded host
row. Capturing that host-menu screenshot on its own shows the compact row, so the preview changes with the selected
neighbors and worker allocation.

scripts/boat/boat-tests.ts:485–496 groups selections and 538 sends a member's whole share to one staged app.
scripts/docs_shots.py:65–81 uses page-file order, so manage-hosts precedes start-a-session regardless of selector order.
scripts/docs-shots/manage-hosts.py:43–47 enables without returning a cleanup callback. scripts/docs_shots.py:296–315
hides Hosts during the next reset, but the app deliberately preserves while hidden
(crates/gui/farhelm-gui/src/hosts.rs:11–16 and 224–226). scripts/docs-shots/start-a-session.py:91–95 reopens Hosts
without changing Details, and 263–273 crops the now-expanded host row together with its menu. The parent of
914eaaf6d030096ebf6d0037f634d17b3d80e96a staged the two pages separately in Boat. The full local capture already shared
this state. Restore in the shot's cleanup, including failure before the callback is returned, or return the panel to its
initial detail state on each screenshot reset. Keep the product's intentional persistence across ordinary panel toggles.

Confidence: confirmed by inspection of state ownership and the capture region. No image render was performed.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `shots/l14`.
