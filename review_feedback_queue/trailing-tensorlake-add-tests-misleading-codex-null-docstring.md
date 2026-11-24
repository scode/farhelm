# tensorlake_add_tests docstring misstates why record.codex is null during Setup and omits read-agent-settings absence check

Reviewed commit: afe81073d040516d0f9e638127d311d4aab80b80

## TLDR

A test docstring in Tensorlake setup tests inaccurately claims a fresh host leaves its Codex cleanup stamp unset because
it has no earlier Codex settings write, when in fact a fresh host records `Cleared` as soon as Codex reconciliation runs
(on connect) and only stays `null` here because setup skips Codex reconciliation before the protocol-87 hello. The test
also omits an assertion that `read-agent-settings` did not run.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/tensorlake_add_tests.rs:1128`.

- In `codex_subscription_and_none_enter_setup_without_an_endpoint`
  (`crates/farhelm-helm/src/provisioning/tensorlake_add_tests.rs:1125-1209`), commit `afe81073` adds the docstring claim
  (lines 1128-1130): "A fresh host has no earlier Codex settings write, so neither mode reads the agent settings or
  records a Codex settings cleanup: only an earlier endpoint write ever calls for one." It then asserts
  `assert!(record["codex"].is_null(), "{record}");` at line 1198.
- That rationale contradicts the actual state machine and `SPEC_impl.md:6111` ("A host with no legacy stamp records
  Cleared directly, preserving its image settings"):
  1. In `crates/farhelm-helm/src/provisioning/credential_delivery.rs:970-1008` (`retire_legacy_settings`), when Codex
     reconciliation runs on a fresh host (`record.codex == None`), `needs_cleanup` is `false` (no settings read), and
     line 1006 unconditionally writes `record.codex = Some(EndpointStamp::Cleared)` (asserted on connect in
     `credential_delivery_tests.rs:1256`).
  2. `record["codex"]` is `null` in `tensorlake_add_tests.rs:1198` solely because `tensorlake_add.rs`
     (`lines 691, 1573, 1680`) runs `DeliveryPhase::Setup`, which sets `codex = None` (`credential_delivery.rs:705-708`)
     and defers all Codex endpoint reconciliation and cleanup stamping until `DeliveryPhase::Connected` after a
     protocol-87 hello.
  3. Additionally, although line 1176 reads `backend/events.jsonl` into `events`, the test never asserts
     `!events.contains("read-agent-settings")` (unlike
     `claude_subscription_setup_delivers_only_to_the_host_before_attach` at line 1253 which asserts
     `!events.contains("wrote-claude-settings")`).
- Risk: A future refactor reading this docstring and `assert!(record["codex"].is_null())` could mistakenly conclude that
  fresh hosts should keep `record.codex == None` rather than stamping `Cleared`.
- Verification: source inspection of `tensorlake_add_tests.rs:1125-1209`, `tensorlake_add.rs:691,1573,1680`,
  `credential_delivery.rs:705-708,970-1008`, `credential_delivery_tests.rs:1256`, and `SPEC_impl.md:6111`.
- Fix: Update the docstring at `tensorlake_add_tests.rs:1128-1130` to state that setup defers Codex endpoint
  reconciliation and legacy cleanup until a compatible supervisor connects (so setup neither reads agent settings nor
  stamps `record.codex`), and assert `!events.contains("read-agent-settings")` alongside the existing event checks.

Reviewer metadata (g22b_codex_recipe_creds_tests/c1): suggested bucket: other (inaccurate test docstring and missing
negative event assertion; no user-facing security or data-loss impact).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g22b_codex_recipe_creds_tests/c1`.
