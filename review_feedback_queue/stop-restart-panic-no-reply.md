# A crashed stop or restart task never answers

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

If the supervisor's stop or restart work hits an internal crash, the Stop or Restart button (or an agent's
`farhelm agent stop/restart`) waits forever instead of showing an error, until the helm's connection to that host drops.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F23 / COR-TASK-PANIC`, tagged **possible**. Anchor and title:
`crates/farhelm-supervisor/src/service/handlers.rs:1452` — a panicking stop or restart task never answers its request.

In the supervisor, Stop, Restart and Delete each run their actual work on a separate supervisor-owned task so a dropped
helm connection cannot interrupt them half way. A second small task waits on the work task. For Delete
(`handlers.rs:1599-1608`), if the work task panics, that waiter still sends the helm an Internal error reply ("the
session delete task failed: …"). For Stop (`handlers.rs:1452-1456`) and Restart (`handlers.rs:2364-2368`), the waiter
only logs the panic and sends nothing for that request id.

The helm deliberately puts no deadline on waiting for a supervisor's reply. So a panic in a stop or restart task leaves
the browser's Stop or Restart spinning, or an agent's `farhelm agent stop/restart` blocked, until the helm-to-supervisor
connection eventually drops. Stop and restart are among the most consequential lifecycle actions, and the helm itself
already treats "a panicking handler still answers" as a requirement for its own agent-request handlers (`panic_fallback`
in `crates/farhelm-helm/src/client.rs`).

The reviewer found no concrete way to make these tasks panic, only the usual invariant checks (a poisoned mutex, a
closed semaphore), which is why this is tagged possible. The fix is small: do what Delete does, passing the request id
and a clone of the reply channel into the stop and restart waiters and, when the join reports a panic, sending an
Internal error. For restart the message should say the outcome is unknown, since the panic may have come after the old
agent was stopped or the new one launched.
