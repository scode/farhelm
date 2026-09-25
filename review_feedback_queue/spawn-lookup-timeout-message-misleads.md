# A spawn profile-lookup timeout implies a session may exist

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A spawn that timed out looking up its profile suggests it might have created a session when it did not.

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F16 / COR-SPAWN-TIMEOUT-MSG`, tagged **definite**. Anchors and title:
`farhelm-supervisor/src/service/handlers.rs:534`, `farhelm-supervisor/src/service/agent_relay.rs:468-482` — A timed-out
profile lookup during spawn says a retry "may or may not repeat the request"

When `farhelm spawn --agent NAME` needs a profile, the supervisor first asks the helm with the `ResolveProfile` upcall.
If the helm does not answer within 30 seconds, the relay produces its generic timeout text: "…it may or may not have
reached the helm … so a retry may or may not repeat the request" (`answer_budget_expired`, `agent_relay.rs:468-482`).
`resolve_restricted_profile` passes that through to the spawning agent unchanged (`handlers.rs:534`).

For this call the sentence is misleading. The lookup runs **before** any idempotency reservation or session creation, so
when it times out, no session exists and a retry is completely free. An agent reading "may or may not repeat the
request" may avoid a safe retry, or go looking for a child that was never made. This is low severity and affects wording
only. The fix is to map relay failures on this path to a spawn-specific message that says no session was created and the
spawn can be retried.
