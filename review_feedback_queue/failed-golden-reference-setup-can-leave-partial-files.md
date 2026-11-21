# Failed reference setup leaves orphaned comparison directories

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A failed reference fetch can strand partial files from a golden comparison that never started rendering.

## Details

scripts/check-gui-goldens.py:248–254 creates a unique compare directory before automatically fetching references. A
failure then skips the only cleanup at :262–265, which runs after a successful renderer return. The ordinary
missing-blob refusal in scripts/gui-goldens.py:231–239 leaves the unique compare directory empty. More consequentially,
the fetch writes the validated references one by one at :245–248. If a local write fails after earlier files were
written, those copied files remain under the orphaned run directory. The failure occurs before any renderer starts, and
a retry allocates a different directory without reclaiming the first. The documented retained-comparison evidence
concerns mismatch captures and their corresponding references (.agents/validation.md:720–724 and
scripts/check-gui-goldens.py:30–32). Keep comparison directories on actual render failures, and clean up the new
directory when reference setup or another pre-render step fails.

Confidence: confirmed by inspection of the fetch exception path and runner cleanup. The disk-usage consequence requires
a failure after at least one reference was copied.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `goldenRunner/l16`.
