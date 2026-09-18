# Refusing UPDATE on the local row returns 500 instead of a 4xx refusal

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Planning an upgrade of the helm's own machine answers "server error" although the operation is deliberately refused by
policy ("run setup here instead") — the same state another endpoint reports as a normal, explained outcome.

## Details

Source: pre-pr-review-swarm, area provisioning, 2026-09-17. Confidence: definite (correctness-state-lifecycle p2).
Coordinator confirmed the untyped refusal and the 200/500 split across endpoints.

`plan_update` refuses every local row with `bail!(self.local_handoff_reason().await?)` (service.rs:597) — untyped, so
`provisioning_error` falls through to 500. The ADD probe returns the identical host state as a 200 `Manual` response
with the same handoff text. The body is actionable but the status contradicts it, misleading clients, retry logic, and
alerting into treating policy as malfunction.

Suggested fix: return a typed refusal mapped to 409 (e.g. a new `ProvisioningRequestError` variant such as
`NotProvisionable(String)` rendered from the handoff reason, with `provisioning_error` mapping it to `CONFLICT`) instead
of untyped `bail!`. Sibling of `manual-update-refusal-returns-500.md`.
