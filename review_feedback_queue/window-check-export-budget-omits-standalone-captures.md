# Window-check exports can omit standalone capture references

Reviewed commit: b1eed46975aab16ccc2217b799d5d9a045d8e979

## TLDR

Ten raw screenshots can exceed the standalone reference budget, leaving fewer exported captures even when the check
passes. Original image bytes remain in digest-bound raw events; total evidence loss is not claimed.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `releasing/mac-vm-test/guest_tester.py:511-518`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit: b1eed46975aab16ccc2217b799d5d9a045d8e979. TLDR: A passing window-drag release check can leave fewer
than its ten promised standalone screenshots in accepted result evidence. guest_tester.py:467-480 silently omits
captures exceeding the 16 MiB aggregate result-evidence budget. The new gate at :511-518 counts ten occurrences and
checks geometry image hashes in raw events; :528-531 requires only one retained result image. Consumer synthesis
establishes that brick_test.py:791-839 passes the reduced evidence list into record; :592-618 requires only one native
image for window-drag (the two-image check is P1 only); :656-666 stores the reduced references and hashes, including the
raw event log. verdict at :1030-1073 rechecks recorded hashes and continuity rather than capture count. Thus the missing
standalone references are not repaired or rejected downstream. The ten-file contract is explicit in
BRICK-TEST.md:320-327, RECIPE.md:180-192, and test_guest_tester.py:104-113 (separate timed image states must survive
export). Existing budget tests at :377-398 exercise ordinary checks and do not require the ten-state intersection.
Suggested bucket: other, bounded maintainer evidence-format consequence. Confidence: confirmed conditional source
mechanism; actual native screenshot aggregate sizes/frequency unmeasured. Caveats: all original image bytes remain in
exported, hash-bound events.jsonl; no total evidence loss, unperformed gesture, bad-release promotion, or
user-work/security consequence is established. This revision narrows the trigger to byte-budget pressure; it does not
assert that normal window checks can exhaust reference slots. possible_cover: BRICK-TEST.md:320 describes the ordinary
bounded retention policy, but :321-327 and RECIPE explicitly single out ten window captures; reading those as only
raw-event retention remains an interpretation caveat, not source refutation. Prior pinned coverage searches found no
matching acceptance or duplicate. No producer/consumer/contract changes occur between this commit and pinned tip.
proposed_drop: none.

Provenance: `g311/f1`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
