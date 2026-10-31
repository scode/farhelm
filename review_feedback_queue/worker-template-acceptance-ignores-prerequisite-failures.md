# Worker acceptance ignores prerequisite failures

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Automated template acceptance can welcome a worker despite failed manager or SSH prerequisites.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The prerequisite checks' failed statuses do not all gate acceptance. The independent source review preserves the missing
readiness contract; no actual cloud template or worker was tested.

Source locations: `scripts/tensorlake/run-shard.sh:46-56; scripts/tensorlake/sandbox-tests.ts:200-205`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis recheck at reviewed d6645f771bee8ed03a66486ef8755834937d759d, original introduced provenance
e9095ef8463dbd438a4a36d5617a3138a564b364. Reread worker setup :1-75, template acceptance :194-218 and recipe :91-113.
The timeout/failed SSH probe is diagnostic only; no prerequisite admission gate prevents welcome tests and checkpoint.
With CI unset (a state run-shard permits rather than forbids), downstream fleet tests skip on absent self-SSH. Finding
concerns acceptance of an incompletely provisioned dedicated worker and consequent missing fleet coverage; it does not
claim skipped tests ran, that generic mode is forbidden, or that all browser tests early-return without systemd. Summary
FAILED/skips remain visible. Confirmed conditional source mechanism, other tooling consequence, no catastrophic claim.
proposed_drop null. possible_cover remains manual human summary check .agents/tensorlake.md:96-97 plus read-results
:45-49; neither unambiguously accepts automated template certification without the stated prerequisites.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g111/f5`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
