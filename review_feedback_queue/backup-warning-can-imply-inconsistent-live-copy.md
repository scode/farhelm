# Backup warning can imply an inconsistent live copy

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The live-WAL backup warning may read as a complete copying recipe even though uncoordinated copies can be inconsistent.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation**. Unverified after investigation; the checked
results and exact remaining premise are preserved below.

Primary SQLite guidance confirms that copying a live database and sidecars without a consistency boundary is unsafe. The
published sentence says to also copy sidecars; it can instead be read as a necessary condition rather than a sufficient
backup procedure. Actual source and wording checks establish the dangerous mechanism but not the editorial causal
reliance needed to classify this statement as a data-loss instruction. Retain other with that exact premise; an explicit
unsafe recipe or established harmful reliance would raise it. Published-material policy coverage also remains ambiguous.

Source locations:
`CHANGELOG.md:27 at d6645f771bee8ed03a66486ef8755834937d759d; a48702321b58474f085c902ecb9885c9ef09e223:29`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Separate discovery pass begins. Parent reports a044's independent review retains the ambiguous wording risk: the SQLite
WAL-copy hazard is mechanically established, while necessary-condition versus sufficient-recipe interpretation of 'also'
remains unresolved. I have not read a044's private report or treated it as runtime proof. Retain f1 open,
proposed_drop=false, suggested other under established-consequence rule, preserving the missing wording-inducement
premise and potentially-highest unsafe-copy/restore consequence if that premise is established. No withdrawal or erasure
of initial overstatement/corrections. Discovery now proceeds beyond this known concern.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g080/f1`, `g081/f1`, `a044/f1`. These are
original reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts
remain in the private review evidence. No fix or triage decision was executed.
