# Rust sandbox runner reports an empty run as passed

Reviewed commit: f17458c5ed6159cac9f621cc2350360d00fa396c

## TLDR

The Rust sandbox runner can announce all shards passed while running no tests, including with zero or nonnumeric shard
counts and an empty overall filter selection.

## Details

Suggested bucket: **other**. Recorder confidence: **verified**. The original reviewer's confidence, qualifications and
severity history are retained below; source-confirmed conditional behavior is not a runtime observation.

The queued failed-shard exit-status item assumes actual shard failures. Zero work accepted as a passing run is a
separate validation claim. Controlled release nextest mode rejects empty selection, but this archive runner uses generic
recording and --no-tests warn.

## Recorded source assessment

Reviewed commit f17458c5ed6159cac9f621cc2350360d00fa396c. TLDR: an empty Rust test selection can be announced as ALL
SHARDS PASSED despite executing no tests. Checked full scripts/tensorlake/rust-tests.ts:1-148, run-rust-shard.sh:1-105,
.config/nextest.toml:1-83, recorder parse_args :225-272 and test_run_nextest.prepare :120-174. Archive shards use
generic recorder mode (:92), not --runner nextest, and explicitly use --no-tests warn (:96), allowing legitimate empty
individual partitions. But the parent aggregates only return codes (:140-143), with no combined selected-test count or
preflight listing. A filter matching nothing across the archive therefore gives only empty successful partitions.
Independent certain zero-work route: main :128 accepts --shards 0 (or a nonnumeric nonempty value converted to NaN);
Array.from length at :141 creates no shard calls and empty codes.every at :143 is true, again producing ALL SHARDS
PASSED. No positive-integer validation exists before create/stage. Suggested bucket other: maintainer coverage claim,
not shipped product. Confidence confirmed for zero/NaN shard route and source-confirmed CLI configuration for
empty-filter route; no runtime invoked. proposed_drop false. possible_cover none checked. g020/c2 correctly refutes
empty-selection success in the controlled release runner, whose --no-tests fail at test_run_nextest.py:166 does not
apply here. Fix: require a finite positive integer shard count and validate total selected/executed tests across shards
while still allowing an individual empty partition.

Provenance: trailing-review run `f57167f6-e8b1-4ac6-ac31-413e21137114`, original record `g045/l5`, exclusive base
`49ce8cb76e9f4c841e15122a89264c7c908171dc`, inclusive tip `9e3ab60b86025ca74948ededd09b99cd81a2477c`. The private
findings history, dispositions, delivery audits and independent verdicts preserve revisions and narrow exclusions. No
fix or triage decision was executed.
