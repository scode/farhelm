# Timing history can retain success after the recorder fails

Reviewed commit: 1446b7d87b6a74a322fe77821c5cd4ba04febbe1

## TLDR

Permanent browser timing history can say the recorder succeeded even though required evidence publication fails and the
recorder returns failure. The child test pass can still be accurate.

## Details

`scripts/test_run_timings.py:120–121` copies recorder outcome and exit code. `scripts/record-test-run.py:1886–1892`
publishes it before the required manifest write (`1893–1894`). A later exception changes the recorder outcome to
`recorder-error`/125 (`1900–1904`) without revising the permanent timing file (`1910–1921`). A writable timing
destination and full/unwritable temporary evidence volume are one possible trigger. Optional timing-write errors are
separately caught and do not cause this issue. Publish after determining the required-write result, or explicitly
separate child and recorder outcomes in the timing schema. Existing tests check counts/identity, not this ordering
boundary.

Suggested bucket: **other**. Confidence: **confirmed conditional mechanism; later publication exception unobserved**. No
product or user-work-loss consequence is alleged. Proposed drop: none.

Possible cover: `SPEC.md:2937–2952`, “Healthy local filesystems,” accepts local failures and stalled progress, but does
not explicitly accept a durable false recorder-success record or clearly apply to this tooling consequence. The rare
diagnostic filter excludes success reported for failure. `TRIAGE_OUTCOMES.md:9119–9135` preserves true child facts after
an earlier publication failure; that is a different fact and branch. No exact Planned, BUGS or queue match was found.
Provenance: `browser-harness-tests-4/f1` with contracts, implementation and other test reviewers' corroboration,
trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `browser-harness-implementation-1/f3`, `browser-harness-tests-1/f3`, `browser-harness-tests-2/f2`,
`browser-harness-tests-3/f3`, `browser-harness-tests-4/f1`, `browser-harness-contracts-1/f1`.
