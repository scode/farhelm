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

## Trailing-review extension through 9e3ab60b

Reviewed commit: f17458c5ed6159cac9f621cc2350360d00fa396c

The Rust sandbox command can exit successfully even when its shard results say tests failed.

Suggested bucket: **other**. Recorder confidence: **verified**. The original reviewer's confidence, qualifications and
severity history are retained below; source-confirmed conditional behavior is not a runtime observation.

Extend the existing browser-runner aggregate-status item. The Rust shard shell does propagate its recorder exit code;
the defect is only the top-level CLI aggregation. The original missing-shard-exit allegation was withdrawn after
complete delivery.

## Recorded source assessment for the Rust runner

Reviewed commit f17458c5ed6159cac9f621cc2350360d00fa396c. Synthesis rechecked complete caller/callee source with
numbered lines: run-rust-shard.sh:92-105 captures and propagates recorder status; rust-tests.ts:97-110 captures r.code,
attempts artifact copying for failure and returns it. Main :140-143 awaits codes and prints failure, then :144-148
sweeps and ends without setting Deno.exitCode or throwing. Corrected line references from prior event (which were
offset): the CLI failure summary is :143, not :140; the shard script exit is :105, not :106. TLDR: shell automation sees
success even after failed Rust tests. Ordinary test-failure artifact copying is attempted; no claim that it is skipped.
Suggested bucket other, confidence confirmed, proposed_drop false, possible_cover none checked. f1 remains withdrawn;
this narrower finding preserves the reduced consequence explicitly. Adjacent durable logs g020/c2 and g046/c1 examine
release nextest selection/retention helper coverage and do not cover or contradict this CLI status boundary. No runtime
checks.

Provenance: trailing-review run `f57167f6-e8b1-4ac6-ac31-413e21137114`, original record `g045/f2`, exclusive base
`49ce8cb76e9f4c841e15122a89264c7c908171dc`, inclusive tip `9e3ab60b86025ca74948ededd09b99cd81a2477c`. The private
findings history, dispositions, delivery audits and independent verdicts preserve revisions and narrow exclusions. No
fix or triage decision was executed.
