# A failed page suppresses final PNG paths in the Boat capture log for successful sibling pages

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

One failed page can hide the paths of sibling screenshots that were captured successfully and kept in the archive.

## Details

scripts/docs-screenshots.sh:5–9 describes printing each PNG's path. scripts/docs_shots.py:434–442 keeps successful crops
and continues after an individual failure. The shell wrapper exits on the combined failure at
scripts/docs-screenshots.sh:142–147 before the selected-path listing at 148–169. The pinned Boat runner invokes the
wrapper once for all of its selectors at scripts/boat/boat-tests.ts:538 and fetches docs.log and the archive at 543–545
even on an engine failure. In 914eaaf6d030096ebf6d0037f634d17b3d80e96a^ at scripts/boat/boat-tests.ts:512–526, each page
produced a separate full log, so a failed page did not suppress the successful sibling page's path listing there. The
terse console summary already filtered those paths, and the per-page mixed-success omission already existed. Report the
names of successfully finalized crops in the full log on partial failure while retaining the nonzero exit and
withholding the publish record. Use the engine's successful crop set or the archive inventory so the report identifies
final preview paths and excludes failed writes.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `shots/l11`.
