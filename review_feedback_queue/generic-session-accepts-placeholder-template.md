# Placeholder resume templates are silently accepted for generic sessions

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

A user who configures a resume template for a non-integrated (generic) session gets no error — but the template can
never work, so every restart offers "start over" with no hint that the configuration is dead.

## Details

Source: pre-pr-review-swarm, area supervisor-launch, 2026-09-17. Confidence: possible. The mechanism is fully verified;
tagged possible because the consequence is silent dead configuration rather than an established failure in a concrete
session.

`IntegrationSnapshot::resolve` (`crates/farhelm-supervisor/src/agent_kind/mod.rs:1945-1963`) enforces only one direction
of the template/kind partition: an integrated kind with a placeholder-free template is rejected
(`SnapshotError::IntegratedTemplateHasNoPlaceholder`, mod.rs:1954-1958), but a `Generic` session with a
`{conversation}`-bearing template is accepted and stored. That template can never take effect: `restart_offer` returns
`Resume` only when `integration().is_some()` (mod.rs:2009) and `FallbackTemplate` only for placeholder-free templates
(mod.rs:2010-2014), so a generic session with a placeholder template always gets `FreshOnly`; and filling needs a
captured id, which a generic session can never obtain (`integration_for(Generic)` is `None`, "the one gate every capture
path passes through", mod.rs:1965-1969). The template is validated strictly at create time and then ignored forever. The
error type documents the partition but enforces half of it, so this reads as an asymmetric-validation gap rather than
deliberate leniency.

Suggested fix: reject the mirror image in `resolve` — when `integration.is_none()` and the template contains the
placeholder, return a `SnapshotError` variant saying a placeholder-bearing template needs an integrated kind (or to drop
the placeholder for a verbatim fallback). Fails fast at create instead of accepting a value restart can never honor.
