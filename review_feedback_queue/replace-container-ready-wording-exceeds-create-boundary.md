# Replace's container-ready wording exceeds its actual boundary

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Container Replace text promises a ready replacement container before deleting the source, while code waits only for the
new session/terminal boundary.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The source and independent reviews establish the ordering-contract mismatch. The actual Replace confirmation authorizes
deleting the source, so this is not an independently established unconfirmed work-loss route. Readiness and destructive
consent remain separate questions. No replacement was started during review.

Source locations: `SPEC.md:3635-3636; crates/farhelm-helm/src/sessions.rs:3711-3713,3821`. Historical revision
qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Cross-finding synthesis supplement; retains the f3-synthesis disposition and proposed highest-claim exclusion. f1 cannot
be combined with f3 on the same replacement attempt to claim a confirmed storage-error-caused deletion of the old
container. f1's confirmation rollback returns Err (supervisor core.rs:11038); do_replace_session propagates the create
error through await? (sessions.rs:3711-3712), so finish_replacement and its source Delete are not reached. f3 instead
concerns a successful session Create followed by a later asynchronous container-preparation failure. f2 changes future
launches from a fleet template, not the stored source launch copied by plain Replace. No compound additional
user-work-loss route was established. Reviewed d6645f771bee8ed03a66486ef8755834937d759d; suggested bucket other;
confidence confirmed ordering/text conflict; proposed_drop only for the stronger unauthorized-work-loss interpretation,
pending independent a004/parent exclusion review; possible_cover SPEC.md:1041-1045,1408-1433 and SPEC_impl.md:4329-4353,
conflicting with SPEC.md:3635-3636.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g112/f3`, `g123/f1`, `g116/f5`, `g150/f3`,
`a005/f2`, `a021/x-g117-support`. These are original reviewer identities for this concern, not additional bugs. Full
revisions, withdrawals and independent verdicts remain in the private review evidence. No fix or triage decision was
executed.
