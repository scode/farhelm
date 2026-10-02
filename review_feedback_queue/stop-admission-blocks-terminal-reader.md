# Stop's separate admission wait can freeze unrelated terminal input

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

A Stop waiting for management capacity can freeze input to unrelated sessions on the same host.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F14 / COR-STOP-ADMISSION`, reviewer `correctness_systems`, pass 1. Confidence: **definite**. Review disposition:
**would surface**. Queue priority at recording: **high**.

Anchor against the reviewed commit: `crates/farhelm-supervisor/src/service/handlers.rs:1158`. Recorded from the
completed review without rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording: No
matching existing queue item or prior decision was identified.

Stop has its own wait for one of the eight shared management slots, performed in the connection reader before its
independently owned mutation starts. If all slots are occupied, the next Stop prevents the reader from dispatching later
input, resize, detach, and status requests for unrelated sessions. This acquisition is separate from the common dispatch
helper, so repairing that helper alone leaves Stop able to freeze the connection.

The accepted stop operation already survives caller loss correctly; the problem is the earlier capacity wait. Give Stop
bounded admission that can queue or refuse without suspending the reader, and retain supervisor ownership and the permit
through completion once accepted. Test saturated admission with Stop followed by input to another session on the same
connection. Queueing stops is allowed, but that queue must not prevent typing into sessions that remain running.
