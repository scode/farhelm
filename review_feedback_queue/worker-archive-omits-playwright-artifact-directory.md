# Worker archives omit Playwright's failure artifacts

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The worker archive can omit the screenshots, traces and timelines Playwright writes outside its archived directory.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Version-matched Playwright source and the configured output paths establish the directory mismatch. The retained
consequence is incomplete maintainer failure evidence, not loss of product user data. No runtime failure archive was
produced here.

Source locations: `scripts/tensorlake/run-shard.sh:64-67; scripts/tensorlake/sandbox-tests.ts:156-163`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g111/f4`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
