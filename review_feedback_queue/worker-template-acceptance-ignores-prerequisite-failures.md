# Worker acceptance ignores prerequisite failures

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Automated template acceptance can welcome a worker despite a failed SSH prerequisite. The manager-readiness case has
been repaired.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The SSH prerequisite's failed status does not gate acceptance. The independent source review preserves the missing
readiness contract; no actual cloud template or worker was tested. The historical assessment below is narrowed by the
later source check.

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

## Narrowing through a98147fb

At reviewed commit `a98147fb2eaaefb448473107529ada69114d7b03`, the partial repair in
`6be1e140190fcbcbfd39a2c8cf544c043b3512b0` makes failed lingering or unavailable manager readiness exit before tests
(`scripts/tensorlake/run-shard.sh:48–53,84–101`). Template acceptance rejects that unsuccessful probe
(`scripts/tensorlake/sandbox-tests.ts:201–206`). The manager-admission allegation is therefore no longer open.

The self-SSH check at `run-shard.sh:108` remains diagnostic-only. Preserve that narrower prerequisite-admission defect
and its earlier caveats; neither the visible failed/skipped summary nor the requirement to read it unambiguously accepts
automatic certification without working SSH. No actual cloud readiness failure or missing test execution was reproduced.

Provenance: trailing-review run `4b58b499-b330-4a1c-841a-38eda725531a`; `g011/f1` identified the stale scope and
`g018/f1` independently found the existing SSH remainder. Both used Codex gpt-6.1-sol, high reasoning. This corrects the
record's scope; it does not fix the remaining worker behavior or execute a triage outcome.
