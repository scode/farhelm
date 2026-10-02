# Fresh-checkout reconciliation also blocks the host's terminal reader behind Delete

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

Starting a fresh checkout during Delete can freeze unrelated terminal input before the actual create request is even
sent.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F12 / COR-RECONCILE-DISPATCH`, reviewer `correctness_systems`, pass 1. Confidence: **definite**. Review disposition:
**would surface**. Queue priority at recording: **high**.

Anchor against the reviewed commit: `crates/farhelm-supervisor/src/service/handlers.rs:2947`. Recorded from the
completed review without rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording:
TODO.md's Planned creation-dispatch item is related, but does not explicitly name the separate reconciliation request.
Keep this dispatch location visible so a create-only fix does not silently omit it.

Before a keyed fresh-checkout create, the helm asks the supervisor whether that request already has a recorded result.
This reconciliation request runs directly in the shared connection reader. It acquires the same intent and host-wide
directory locks as creation, even when it ultimately reports that the key is unknown. Because Delete holds the directory
lock through process teardown, starting a fresh GitHub checkout during a deletion can block the reader before the helm
even sends the create request. Later input, resize, detach, and list requests cannot be dispatched.

Moving ordinary creation out of the reader would leave this separate freeze intact. Run reconciliation as tracked work
outside the reader, with bounded admission that does not suspend dispatch, while preserving the locks that make recovery
and replay safe. Exercise the actual reconciliation request while Delete holds directory admission, and verify
independent terminal input and session listing before releasing Delete. Management requests may wait for each other;
unrelated interactive traffic must not inherit that wait.
