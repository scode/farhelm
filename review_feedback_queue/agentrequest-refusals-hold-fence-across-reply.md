# AgentRequest refusals hold the per-session fence across the reply send

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

A client that sends a request and then stops reading can freeze its own session: the refusal gets stuck behind the
unread replies while holding a lock, so the session can no longer be deleted and its later requests never complete.

## Details

Source: pre-pr-review-swarm, area supervisor-handlers, 2026-09-17. Confidence: definite (correctness-state-lifecycle
p1). Coordinator confirmed the guard lifetime, the reply contract, and the waiters; the restater corrected the
arm→reason mapping and the coordinator verified the correction.

Mutating agent requests are serialized by a per-session fence, claimed up front (handlers.rs:3341). On the success path
the fence moves into the relay, which releases it correctly. On all four refusal exits — validation failure (3363),
wrong-identity (3380), bad credential (3389), credential-check error (3394) — the fence stays alive in the arm frame
across `send_reply` (3399). The fence guard holds an async mutex (`KeyedGuard` over `OwnedMutexGuard`,
core.rs:1617-1623), and `send_reply` awaits the bounded writer queue (connection.rs:935-936) under an explicit contract:
"It must NOT be called while a supervisor mutex is held" (932-934). Session delete claims the same fence first with no
timeout (1254), as does every later mutating request for the session.

Suggested fix: drop the fence before the reply on all four refusal exits — or restructure so refusals are built before
the fence is ever claimed, which becomes natural once validation moves above the claim (see
`agent-fence-claimed-before-validation.md`).
