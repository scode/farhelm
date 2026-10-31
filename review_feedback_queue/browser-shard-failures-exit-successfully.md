# Failed browser shards leave a successful command

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The browser harness can exit successfully even when its shards report validation failures.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The inspected child-status/result aggregation does not propagate failed browser validation to the outer command's exit
status. This is false success in maintainer tooling, not a product test result observed during this review.

Source locations: `scripts/tensorlake/sandbox-tests.ts:149-161,185-192,230-232`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis recheck at reviewed d6645f771bee8ed03a66486ef8755834937d759d, original introduced provenance
e9095ef8463dbd438a4a36d5617a3138a564b364. Nonzero attached exec result is captured as a value, returned by shard, then
only used to choose a log string. run/main never propagate aggregate failure. If cleanup succeeds, ordinary test
failures yield a successful dispatcher process status, despite visible SOME SHARDS FAILED text. Confirmed source
mechanism; other tooling consequence, no catastrophic or product-data-loss claim. proposed_drop null; possible_cover
none. Human instruction to read summaries is a backstop and does not correct process status; this is not the different
queued permanent-timing-record defect. No runtime pass or failure asserted.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g111/f2`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
