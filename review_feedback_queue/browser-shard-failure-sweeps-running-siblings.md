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

## Trailing-review extension through 9e3ab60b

Reviewed commit: f17458c5ed6159cac9f621cc2350360d00fa396c

A Rust runner infrastructure or local evidence-save exception can sweep sibling sandboxes before their work and evidence
collection settle.

Suggested bucket: **other**. Recorder confidence: **verified**. The original reviewer's confidence, qualifications and
severity history are retained below; source-confirmed conditional behavior is not a runtime observation.

Extend the existing browser-runner item: the Rust runner repeats its same Promise.all rejection/global sweep defect.
Ordinary nonzero test statuses are values and do not cause this early sweep.

## Recorded source assessment for the Rust runner

Reviewed commit f17458c5ed6159cac9f621cc2350360d00fa396c. Synthesis confirms finding with corrected numbered references.
rust-tests.ts:95 throws on failed create before the shard try; :100-101 can throw saving local output. Promise.all at
:140-142 rejects immediately on either while sibling shard calls remain in progress. Outer finally :144-146 invokes
sandbox-tests.ts:129-136 sweep, which terminates every still-live ledger sandbox. Per-shard collection :102-107 has not
necessarily run on siblings. TLDR: one infrastructure/local-save failure aborts other shards and can delete their
uncollected failure evidence, including a sibling that completed tests but is still copying. This route is independent
of f1: ordinary nonzero test statuses are values, not rejections, and do NOT trigger early sweeping; that consequence is
explicitly excluded. Suggested bucket other; confidence confirmed by available source; proposed_drop false;
possible_cover none checked. g020/g046 durable records add no duplicate or contradictory lifecycle premise. Fix remains
to settle all started shard operations and collect evidence before the global sweep. No runtime checks.

Provenance: trailing-review run `f57167f6-e8b1-4ac6-ac31-413e21137114`, original record `g045/l1`, exclusive base
`49ce8cb76e9f4c841e15122a89264c7c908171dc`, inclusive tip `9e3ab60b86025ca74948ededd09b99cd81a2477c`. The private
findings history, dispositions, delivery audits and independent verdicts preserve revisions and narrow exclusions. No
fix or triage decision was executed.
