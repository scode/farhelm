# Container policy is absent from agent approval cards

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

An agent's approved template change can alter container isolation without showing that change in its approval card.

## Details

Suggested bucket: **highest**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The authenticated supervisor wire route can submit a template container edit even though ordinary CLI flags omit that
field. The helm approval summaries omit the container choice that the accepted write persists. This is an established
conditional consent mismatch, not a claim that an arbitrary unauthenticated caller can edit templates. Preserve the
distinction between direct CLI omission and protocol admission. See the original reviewers' approval-builder,
template-write and wire-producer references. No runtime reproduction was performed.

Source locations: `crates/farhelm-ui/src/approvals.rs:1184-1368`. Historical revision qualifiers in those references are
retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g112/f2`, `g113/f1`, `a003/g113/f1`, `a005/f5`.
These are original reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and
independent verdicts remain in the private review evidence. No fix or triage decision was executed.
