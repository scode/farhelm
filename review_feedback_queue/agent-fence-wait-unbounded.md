# A mutating agent request waits unboundedly for the delete fence

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After one slow agent action, the next change from that session can hang for up to ten minutes and then happen
unexpectedly.

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F4 / COR-FENCE-WAIT`, tagged **definite**. Anchors and title:
`farhelm-supervisor/src/service/handlers.rs:3644-3649`, `farhelm-supervisor/src/service/agent_relay.rs:325-344`,
`farhelm-supervisor/src/service/core.rs:317`, `farhelm-supervisor/src/service/connection.rs:555`,
`farhelm/src/main.rs:1460-1469` — A mutating agent request can wait up to ten minutes for the delete fence, then run
after its caller gave up

Mutating agent verbs are stop, rename, restart, create, and clone. When a session sends one, the supervisor first takes
a per-session lock called the **delete fence** (`agent_request_locks`). Deleting the asking session waits on the same
lock, which keeps the session's credential from being revoked while its request is in flight. The fence is taken with a
plain `claim(...).await` and no deadline (`handlers.rs:3644-3649`). `KeyedLocks` has a `claim_before(deadline)` variant,
and other supervisor paths use it, but this one does not.

The fence can stay held long after the request that took it. The upcall has a 5-second budget to queue the request to
the helm and a 30-second budget for the answer. If a mutation's answer misses its 30 seconds, the relay deliberately
keeps the fence. A background task holds it until the helm eventually answers or the connection dies, and only after
`AGENT_FENCE_RETAIN_TIMEOUT` (600 seconds, `core.rs:317`) does it give up and tear the connection down
(`agent_relay.rs:325-344`). The reasoning is that a missed budget does not mean the helm stopped working.

The problem is what happens to the **next** mutation from the same session in that window. It blocks on the fence for up
to ten minutes, and none of the 5- or 30-second budgets has started yet, because they only apply once the fence is held.
Meanwhile the handler for a session-authenticated connection runs inline on that connection's read loop
(`connection.rs:555`), so nothing reads the socket. If the calling `farhelm agent` process gives up and exits, the
supervisor cannot notice. When the fence finally frees, the abandoned request is still validated and relayed. The CLI
side has no timeout at all (`main.rs:1460-1469`): its docs say this is deliberate because the supervisor bounds every
request, and SPEC_impl's version-13 section describes the same division of labor.

In practice, agent tools often kill a command after a minute or two. A stop, restart, create, or clone the agent thinks
it abandoned can then take effect minutes later, against state that has since changed. For create or clone, that means a
new session nobody was told about.

The suggested fix is to take the fence with `claim_before` and a deadline, and to answer `Unavailable` when the deadline
passes. That is safe to retry, because nothing was sent to the helm. Optionally, a queued request could be dropped if
the asker hangs up before it is relayed.
