# Worker allocation precedes cleanup ownership

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A created worker can fall outside cleanup when ledger writing fails or remote creation succeeds before the local request
times out.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The worker is allocated before its returned identity is durably entered into the local ownership ledger. A local ledger
error or an accepted remote create followed by response timeout can leave no cleanup entry. Matching CLI source confirms
both boundaries; healthy local storage does not eliminate the latter. Manual recovery needs the returned/retained actual
identity; an unconditional kill by requested name could target foreign work. No worker was created or killed.

Source locations: `scripts/tensorlake/sandbox-tests.ts:60-66,145-146,195-197`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis recheck at reviewed d6645f771bee8ed03a66486ef8755834937d759d, original introduced provenance
e9095ef8463dbd438a4a36d5617a3138a564b364. Reread create, ledger-only sweep and caller try boundaries. Successful
allocation followed by rejected ledger append exits before either caller's cleanup try and leaves no row for sweep, even
with one sandbox. This is loss of tooling resource ownership and possible manual cleanup/billing, other bucket; no
catastrophic spending/product-work-loss claim. Confirmed conditional source mechanism. proposed_drop null;
possible_cover remains Healthy local filesystems SPEC.md:3437-3452, ambiguous for remote allocation ownership rather
than only host-local progress. Recipe .agents/tensorlake.md:24-29 still makes the ledger the required attribution basis.
Parent can judge that narrow possible cover without reclassifying this as highest.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g111/f9`, `a057/f2`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
