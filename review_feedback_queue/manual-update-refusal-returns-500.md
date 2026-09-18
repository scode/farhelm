# UPDATE planning for a manual-needs host returns 500 instead of a 4xx refusal

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Planning an upgrade of a host that needs manual installation answers "server error" instead of the explained refusal —
misleading automated clients into retrying something that needs a human, and firing false alerts for ordinary fleet
states.

## Details

Source: pre-pr-review-swarm, area provisioning, 2026-09-17. Confidence: definite (correctness-state-lifecycle p2).
Coordinator confirmed the untyped refusal and the ADD/UPDATE status split.

`plan_update_unguarded` handles `ReachOutcome::Manual` (no usable systemd user manager, no payload for the architecture)
with `bail!(reason)` (service.rs:703) → 500, while ADD reports the same condition for the same host as a 200 `Manual`
response with the same string.

Suggested fix: the same typed 4xx refusal as `local-update-refusal-returns-500.md` (a `ProvisioningRequestError` variant
carrying the reach reason, mapped to 409) instead of `bail!`. Do not add a 200 `Manual` shape to the update-planning
response unless the API contract is intentionally extended.
