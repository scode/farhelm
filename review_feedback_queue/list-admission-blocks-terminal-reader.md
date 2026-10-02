# Exhausted management slots stop even the session-list dispatcher from reading keystrokes

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

When eight management operations occupy the host, a session-list request can stop later keystrokes and terminal control
messages from being dispatched.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F13 / COR-LIST-ADMISSION`, reviewer `correctness_systems`, pass 1. Confidence: **definite**. Review disposition:
**would surface**. Queue priority at recording: **high**.

Anchor against the reviewed commit: `crates/farhelm-supervisor/src/service/connection.rs:1026`. Recorded from the
completed review without rechecking code after rebasing onto main.

Corroborating review: correctness_state_lifecycle p2, same admission helper plus callers separately retained as F14–F16.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording: No
matching existing queue item or prior decision was identified.

The supervisor limits shared request handlers to eight concurrent operations. Its common dispatch helper waits for a
free slot before starting a task, and that wait occurs inside the connection reader. Eight Stop, Restart, or Delete
operations can occupy every slot while shutting down processes or waiting for lifecycle and directory locks. When a
routine session-list request arrives next, the reader waits for capacity and stops dispatching subsequent terminal
input, resize, detach, upload cancellation, and agent-response messages.

The work limit is useful, but its waiting point lets legitimate management activity freeze unrelated terminals for
shutdown grace periods. Keep admission bounded without waiting in the reader: promptly refuse requests when capacity is
exhausted, or use an explicitly bounded management queue whose worker owns the wait. Preserve request/reply matching and
cleanup, and ensure listings meet their separate responsiveness requirement. Test all slots occupied by controlled
lifecycle operations, followed by a list request and terminal input on the same connection; input must reach the pane
before those operations release their slots.
