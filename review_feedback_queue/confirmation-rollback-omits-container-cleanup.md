# Create confirmation rollback omits container cleanup

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A failed final create confirmation can remove a borrowed-checkout session row without performing its container cleanup;
an ordinary uncovered failure producer remains missing.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

The conditional cleanup omission is established, but source checks did not establish a healthy-filesystem confirmation
error outside the specifically accepted storage-fault combinations after startup, followed by successful borrower-row
removal. Last-reference managed-checkout protections remain intact. Do not mistake an unobserved runtime for the missing
premise or conflate this with the separately complete early-validation orphaning and initialized-file-loss routes.

Source locations: `crates/farhelm-supervisor/src/service/core.rs:10937-11020`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Discovery-turn startup correction from parent/a005: abandonment must involve another retained checkout member; removing
the last reference is refused. Narrow the earlier existing-managed-checkout scope accordingly. This is parent-supplied
reconciliation, not independently reverified here. The skipped container cleanup remains source-confirmed; a supported
non-covered confirmation-error producer remains missing. Suggested bucket other conditional on that producer; previous
filesystem-acceptance and two-fault caveats remain. Use a005, not a004, for cleanup/Replace reconciliation. No new
finding or stronger consequence is asserted.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g112/f1`, `g115/f1`, `g116/f3`, `g114/f3`,
`g122/f2`, `g123/f2`, `g124/f3`, `a004/x9`, `a005/f1`, `a007/e2`, `a023/f2`. These are original reviewer identities for
this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the private review
evidence. No fix or triage decision was executed.
