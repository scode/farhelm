# Rename can freeze the terminal reader while waiting for a management slot

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

Even a title change can freeze typing across the host when other management operations occupy its request slots.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F16 / COR-RENAME-ADMISSION`, reviewer `correctness_systems`, pass 1. Confidence: **definite**. Review disposition:
**would surface**. Queue priority at recording: **high**.

Anchor against the reviewed commit: `crates/farhelm-supervisor/src/service/handlers.rs:2460`. Recorded from the
completed review without rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording: No
matching existing queue item or prior decision was identified.

Rename waits for a shared handler slot inside the connection reader before starting its tracked task. With eight
lifecycle operations occupying those slots, even a title change prevents later terminal input and control messages from
being dispatched. Rename carefully passes one permit through its database update and reply, but that handoff does not
address the initial reader-blocking wait.

A lightweight rename can consequently pause typing across the host for another session's Stop, Delete, or Restart grace
period. Preserve the single-permit handoff while changing admission to prompt refusal or bounded queued work that leaves
the reader free. Test Rename under saturated admission, followed by input to an unrelated session, and verify the input
progresses before lifecycle work releases capacity. Fixing the other admission sites does not fix this separate path.
