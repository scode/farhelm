# A later page can overwrite the screenshot frame retained for an earlier failed page

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A later shot can overwrite the raw image retained for a failed capture, leaving its error linked to the wrong frame.

## Details

scripts/gui-agent.py:1588 and 1906–1929 use one capture.png per app run and remove it before each new screenshot. If a
post-screenshot check fails, such as the dimension check at 1922–1925, the failure explicitly names that file, and the
per-shot full PNG has yet to be written. scripts/docs_shots.py:419–441 keeps the error text and continues without saving
the rejected frame to a shot-specific evidence path. The final run directory is printed at 493–495, and
scripts/gui-agent.py:2746–2813 leaves the PNG alone at stop. Before 914eaaf6d030096ebf6d0037f634d17b3d80e96a, the Boat
collector called the wrapper independently for each page (parent scripts/boat/boat-tests.ts:511–526), so a frame from
the last attempted shot on one page could remain in its own run directory while the next page ran. At pinned
scripts/boat/boat-tests.ts:538, subsequent pages on a member share that file. This also affects a local selection that
spans several pages. docs/docs-shots/SPEC.md:69–71 describes retention of the failed run's full capture and failure
messages. Use a unique raw path for each attempted shot and reference it in the failure, or move the rejected current
frame to a shot-specific private evidence path before continuing. Keep rejected frames out of preview and publish
outputs.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `shots/l17`.
