# The agent fence is claimed before pure validation

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

One malformed request can stall everything on its connection — other sessions' requests included — for up to ten
minutes, only to be refused as invalid with no shared state ever touched.

## Details

Source: pre-pr-review-swarm, area supervisor-handlers, 2026-09-17. Confidence: definite (correctness-state-lifecycle
p1). Coordinator confirmed the ordering, the claim's unbounded wait, and the read-loop context.

The `AgentRequest` arm claims the per-key fence (3341 — no timeout, retainable up to 600 s per
`AGENT_FENCE_RETAIN_TIMEOUT`, core.rs:316) before running `validate_agent_verb` (3358), a pure synchronous shape check.
The arm runs inline in the connection's read loop (`handle_restricted_control(...).await`, connection.rs:549). So a
malformed mutating-shaped request queues behind whatever holds the fence and parks the entire connection's read loop
there — every other request on that connection, for every session — to produce a refusal that needed no serialization at
all.

Suggested fix: move validation above the fence claim; claim the fence only for validated mutating verbs that will
actually relay. Pairs with `agentrequest-refusals-hold-fence-across-reply.md`, whose fix becomes natural once refusals
precede the claim.
