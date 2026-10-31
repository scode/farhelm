# Worker evidence is deleted after a failed download

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The harness can delete a failed worker's evidence even when downloading that evidence failed.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The cleanup path proceeds after the fallible evidence transfer without preserving the only remote copy. This concerns
failure evidence retention in tooling; no remote worker was allocated or deleted during review.

Source locations: `scripts/tensorlake/sandbox-tests.ts:153-164`. Historical revision qualifiers in those references are
retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis recheck at reviewed d6645f771bee8ed03a66486ef8755834937d759d, original introduced provenance
e9095ef8463dbd438a4a36d5617a3138a564b364. Reread archive/copy/status handling and cleanup. All collection commands
ignore failed return codes, but destructive termination and snapshot sweep proceed. A network copy failure suffices; it
does not require broken local filesystem or sibling cancellation. It deletes this run's owned failure evidence, other
tooling consequence with no catastrophic claim. Confidence confirmed conditional source mechanism. proposed_drop null;
possible_cover remains SPEC.md:3437-3452 Healthy local filesystems only for local-I/O subset; no exact accepted
network-transfer-plus-deletion route found. .agents/tensorlake.md:45-49 explicitly expects retained failed evidence,
reaffirmed in synthesis.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g111/f3`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
