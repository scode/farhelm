# An invalid Boat member limit discards previews before rejecting the run

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

An invalid Boat worker limit can erase selected previews and the publish record before any capture starts.

## Details

The CLI accepts members as a string at scripts/boat/boat-tests.ts:651–656. runDocs only verifies screenshot selectors
before removing output and packaging at 569–574. A nonnumeric members value becomes NaN at 575. In spread at 486–496,
that yields zero lists and the nonempty selected-page loop attempts to append to an absent list. The failing calculation
is before the try/finally at 577–594, so the newly created pack stays in scratch. In
914eaaf6d030096ebf6d0037f634d17b3d80e96a^, lines 538–545 passed the numerical limit to claimUpTo, whose loop fell back
to claiming one, and did not clear prior output up front. Validate an explicit member limit as a positive integer and
calculate the desired split before cleaning outputs or building the archive. This is separate from selector rejection
l1.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `shots/l12`.
