# One failed reference lookup hides available originals from the update report

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

One unavailable screenshot reference can hide other originals that were already cached.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d scripts/golden_changes_page.py:44-65,83-93,126-170;
scripts/gui-goldens.py:22-39,181-190,201-216,222-249; .agents/validation.md:699-714,729-739

When an update needs several old pictures, one missing reference can make the report show Reference unavailable for
every existing scene in the batch, even when the other originals are still cached. A scene without a manifest ID is an
intentionally absent baseline: update omits it from the fetch subset, labels an unpaired row New scene, and does not
fetch at all for a new-scenes-only report. A real lookup failure is different. When an expected ID is absent, fetch
checks the remote; a remote error or a still-missing expected ID prevents any reference directory from being
materialized. The report logs the failed fetch once and has no fallback to the unaffected cached originals. The strict
fetch contract is intentional and the compare runner requires it before rendering. The optional update report applies no
completeness requirement to a directory supplied through FARHELM_GOLDENS_DIR, checking each old file against the
manifest independently. Pinned docs describe fetching a copy elsewhere or for a saved manifest, and setting
FARHELM_GOLDENS_DIR to that copy. Those existing pieces can recover unaffected references on a rerun if a caller
supplies a copy or materializes a narrower manifest of available IDs. The failed report neither performs that recovery
nor gives targeted guidance for it. Keep the strict store contract and salvage available verified originals in the
optional update report. This does not establish that any picture is currently missing from the store. Introduced at
0d52ff68e92cd2bee5b4752d02c3a3210ffbdd5d.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `golden-report/GR-04`.
