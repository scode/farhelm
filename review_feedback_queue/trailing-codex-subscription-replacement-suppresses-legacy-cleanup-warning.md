# Codex Subscription replacement suppresses pending Codex legacy-cleanup warnings

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

When a host upgraded from an older Farhelm still has a failing legacy Codex settings cleanup (leaving the old `farhelm`
provider selected in `~/.codex/config.toml` or leaving `config.toml` unparseable), replacing a Codex Subscription login
in Settings -> Credentials reports a clean save with no failure for that host, even though Codex on that host still
cannot use the replaced subscription login until the cleanup succeeds.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/credential_delivery.rs:76`.

In `crates/farhelm-helm/src/provisioning/credential_delivery.rs:76-82`, `Bindings::uses_codex_endpoint` matches only
`self.codex.as_ref().and_then(|endpoint| endpoint.token_credential.as_deref()) == Some(name)`, deliberately excluding
`self.codex_login_credential`. In `CredentialDelivery::push_one`
(`crates/farhelm-helm/src/provisioning/credential_delivery.rs:628-635`), `warnings.codex` is filtered by
`uses_codex = bindings.uses_codex_endpoint(name)` when building `DeliveryFailure`. By contrast, `Bindings::uses_claude`
(`credential_delivery.rs:85-92`) matches both `claude_login_credential` and `claude.token_credential`. When a host was
stamped with a legacy Codex API endpoint (`record.codex = Some(EndpointStamp::Endpoint { .. })`), switched its recipe to
Codex Subscription (`codex = None`, `codex_login_credential = Some(name)`), and
`retire_legacy_settings(..., LegacySettings::Codex, ...)` fails (e.g., invalid TOML or concurrent edit conflict),
`apply()` records `warnings.codex` and still writes `~/.codex/auth.json`. On a subsequent replacement of
`codex_login_credential`, `push_one` computes `uses_codex == false` and returns `failure = None`, so
`PUT /api/credentials/{name}` returns `delivery_failures: []`. This contradicts `SPEC_impl.md` ("Named credential
catalog", line 6116: "The host warning names the affected agent, and credential replacement reports the warning when
that agent's selection uses the replaced name") and hides from the save reply that `~/.codex/config.toml` still selects
the old `model_provider = "farhelm"` (with `requires_openai_auth = false` and old bearer token) or fails TOML parsing,
preventing Codex from using the replaced Subscription login. (Before `eddd763f`, `clear_codex` returned an error before
`codex_auth` via `?`, so replacing `codex_login_credential` reported the failure.) Fix: rename/expand
`uses_codex_endpoint` to `uses_codex` including `self.codex_login_credential.as_deref() == Some(name)` (matching
`uses_claude`), and add a unit test in `credential_delivery_tests.rs` replacing a Codex Subscription login while legacy
Codex cleanup is pending.

Reviewer metadata (g22_codex_recipe_creds_helm/f1): suggested bucket: other — the host itself still displays the amber
warning in Hosts details and the trigger requires a pre-protocol-87 host switching from Codex API endpoint to
Subscription with a failing `~/.codex/config.toml` cleanup followed by replacing the Subscription login; it does not
leak credentials or destroy user data (it causes a misleadingly clean save reply while the old provider remains selected
on a host already flagged amber).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g22_codex_recipe_creds_helm/f1`.
