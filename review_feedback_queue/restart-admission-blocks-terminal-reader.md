# Restart has the same reader-blocking admission wait independently of Stop

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

A Restart waiting for management capacity can freeze input to unrelated sessions on the same host.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F15 / COR-RESTART-ADMISSION`, reviewer `correctness_systems`, pass 1. Confidence: **definite**. Review disposition:
**would surface**. Queue priority at recording: **high**.

Anchor against the reviewed commit: `crates/farhelm-supervisor/src/service/handlers.rs:2340`. Recorded from the
completed review without rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording: No
matching existing queue item or prior decision was identified.

Restart directly waits for a shared management slot before starting its supervisor-owned task. That wait takes place in
the connection reader. When other lifecycle operations occupy all eight slots, a pending Restart prevents later input,
resize, detach, and status messages from reaching unrelated sessions. This is an independent acquisition from both Stop
and the common dispatch helper.

A restart may wait behind other management operations, but SPEC.md does not allow that wait to stop interactive traffic
during their process teardown. Apply bounded admission at this dispatch point without suspending the reader, preserving
supervisor ownership once the restart is accepted. Cover the saturated-restart case with terminal input on the same
connection and require that input to progress before the occupied slots are released.
