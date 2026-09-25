# An AgentRequest without a credential is never answered

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A client that sends an agent request without a credential hangs instead of getting an error.

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F15 / COR-NOCRED-AGENTREQ`, tagged **definite**. Anchors and title:
`farhelm-supervisor/src/service/handlers.rs:3068` — An `AgentRequest` sent without a session credential gets no reply

A connection whose hello carries no session credential gets **full authority**. That is how the helm connects, along
with any other local tool. On such a connection, `AgentRequest` is meaningless, because an agent request must be made as
a particular session. The full-authority dispatcher has no arm for it, so it falls into the catch-all at
`handlers.rs:3068`, which only logs "unexpected control message at supervisor" and sends nothing. The sender waits
forever.

The same dispatcher has an explicit arm for the analogous `ReportConversation` case, with a comment explaining why: a
request with no reply leaves its sender hanging and gives tests nothing to assert. No in-tree client sends an
`AgentRequest` without a credential today, so this is a latent robustness gap rather than a live bug. The fix is to add
an explicit arm that answers `AgentResponse` with an `Unauthorized` error, modeled on the `ReportConversation` arm.
