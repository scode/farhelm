# A rolled-back host number can transfer a retained approval

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

After a particular registry rollback, a different connected host can finish an earlier host's retained, approved launch
using the same request.

## Details

Suggested bucket: **highest**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Retry-key derivation uses the numeric asking-host row and preserved registry namespace, rather than its entry UUID. Loss
of a recent row and sqlite_sequence update can let a newly registered entry reuse that number. A target failure after
its Pending commit but before allocation leaves an unlaunched generation-zero attempt that ordinary recovery can resume.
The original hostile origin knows its own raw key/session/exact request and can share that tuple with a colluding
replacement supervisor; an unrelated victim's key is not claimed learnable. The user must register/connect the
replacement, the same target and exact request must resolve, and current permission/wrapper checks still apply.
Completed attempts only replay their result. SPEC_impl.md's broad recent-state-loss acceptance does not clearly waive
the origin restriction. This is a conditional source route, not arbitrary-command admission or an observed attack.

Source locations:
`crates/farhelm-helm/src/store.rs:5910-5943; crates/farhelm-helm/src/agent_requests.rs:1623-1645,1945-2026`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a040/f1`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
