# Browser shard failure sweeps running siblings

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

An early shard exception can clean up workers that sibling shards are still using.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The outer failure path invokes shared cleanup before all concurrently running shard work has settled. This is a
maintainer test-harness lifecycle defect; no production host cleanup or catastrophic tooling consequence was
established.

Source locations: `scripts/tensorlake/sandbox-tests.ts:185-190`. Historical revision qualifiers in those references are
retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis recheck at reviewed d6645f771bee8ed03a66486ef8755834937d759d; original introduced provenance
e9095ef8463dbd438a4a36d5617a3138a564b364 retained. Reread dispatcher :130-252 and cleanup :98-105. Promise.all still
returns at first thrown error while mapped sibling shards remain pending; run finally sweeps their recorded remote
resources without a settlement barrier. Throwing provision copy/exec or local results write is a concrete source
producer; no arbitrary user-event interleaving or private runtime claim is needed for the conditional mechanism.
Affected work is this run's owned test shards/evidence, other bucket, no catastrophic claim. Distinct boundaries: f1
coordinates sibling completion; f3 handles failed transfer after a shard finished; f4 omits required browser files even
on successful transfer. Confidence confirmed conditional source behavior. proposed_drop null; possible_cover none. Prior
exact coverage searches and no-runtime caveat remain. No lifecycle regression test exists in added :236-252 tests.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g111/f1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
