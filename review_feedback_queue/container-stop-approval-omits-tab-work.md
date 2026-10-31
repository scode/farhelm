# Container Stop approval omits independent tab work

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

An agent Stop approval describes stopping the agent, although it can also terminate independent work in that container's
terminal tabs.

## Details

Suggested bucket: **highest**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The shipped agent-request Stop producer reaches whole-container stopping. The card's agent-only description does not
disclose termination of the other terminal tabs in that container. SPEC.md's later deletion of detached work is a
different operation and does not clearly authorize this Stop consequence. The source route and producer are confirmed
conditionally; no running session was stopped during review. This concerns approval scope, separately from the wrapper
guide's unconditional tab-preservation advice.

Source locations:
`crates/farhelm-ui/src/approvals.rs:836,977-985; crates/farhelm-supervisor/src/service/sweep.rs:1986-1987`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g117/f3`, `a004/x2`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
