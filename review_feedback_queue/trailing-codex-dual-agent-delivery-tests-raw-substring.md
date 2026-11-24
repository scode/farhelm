# Dual-agent delivery tests check raw agent-credentials substrings and omit Codex bearer-absence checks when Codex is withheld

Reviewed commit: eddd763f544dbd8eea93406db1f920f3e4e9a911

## TLDR

Several credential-delivery tests that share one endpoint token between Claude and Codex either check only whether a
token appears anywhere in the raw combined credentials file after recovery, or check only that `codex_endpoint` is
absent without verifying `FARHELM_CODEX_ENDPOINT_TOKEN` was also stripped. A regression leaving one agent's credentials
missing—or leaving an orphaned Codex bearer that causes the supervisor to reject all spawns—would still pass these
tests.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/credential_delivery_tests.rs:629`.

- `crates/farhelm-helm/src/provisioning/credential_delivery_tests.rs:185-188` adds `Fixture::snapshot()` specifically to
  "Decode the file delivered for the next spawn, so a token shared by Claude and Codex cannot mask a missing slot."
  Because `Fixture::snapshot()` only runs `serde_json::from_slice`, it does not enforce the supervisor's
  `RecipeCredentials::validate()` invariant (`crates/farhelm-supervisor/src/recipe_credentials.rs:90-98`) that
  `codex_endpoint.is_some_and(|e| e.has_token) == env.contains_key(CODEX_ENDPOINT_TOKEN_ENV)`.
- Recovery substring checks that allow a shared token to mask a missing slot:
  1. `stamped_claude_cleanup_gates_only_its_launch_environment`
     (`crates/farhelm-helm/src/provisioning/credential_delivery_tests.rs:534-631`): `Fixture::new(true)` shares `"work"`
     between Claude and Codex. Lines 563-568, 582-587, and 609 decode `f.snapshot().await`, but after clearing the fault
     and pushing `THIRD`, line 629 still asserts `assert!(f.claude().await.contains(THIRD));` on the raw JSON text.
     Because Codex also writes `THIRD` under `FARHELM_CODEX_ENDPOINT_TOKEN`, this passes even if Claude's
     `ANTHROPIC_AUTH_TOKEN` and `ANTHROPIC_BASE_URL` were never restored.
  2. `backend_failures_leave_saved_values_and_safe_warnings`
     (`crates/farhelm-helm/src/provisioning/credential_delivery_tests.rs:1731-1796`): `Fixture::new(true)` shares
     `"work"` with a legacy Codex stamp. For `operation` in `["read-home", "read-agent-settings", "write-unit"]`, Claude
     already receives `SECOND` during the failed `push("work")` at line 1775 while Codex is withheld. After clearing
     `*f.backend.fail` (line 1786) and pushing `"work"` (line 1789), line 1794 only checks
     `assert!(f.claude().await.contains(SECOND));`, which was already true from Claude's entry before recovery and never
     verifies that Codex's `codex_endpoint` or `FARHELM_CODEX_ENDPOINT_TOKEN` reached the snapshot.
- Missing `FARHELM_CODEX_ENDPOINT_TOKEN` absence assertions when Codex is withheld or cleared: 3.
  `codex_cleanup_failure_preserves_the_claude_slot`
  (`crates/farhelm-helm/src/provisioning/credential_delivery_tests.rs:1612-1616`) checks raw
  `assert!(f.claude().await.contains(FIRST));` and `assert!(f.snapshot().await.codex_endpoint.is_none());`, without
  checking `snapshot.env["ANTHROPIC_AUTH_TOKEN"] == FIRST` or `!snapshot.env.contains_key(CODEX_ENDPOINT_TOKEN_ENV)`. If
  `credential_delivery.rs:897-899` stopped stripping `CODEX_ENDPOINT_TOKEN_ENV` when withholding Codex, the resulting
  file would be rejected by `RecipeCredentials::read` on the supervisor (blocking all spawns), yet this test would pass.
  4. `backend_failures_leave_saved_values_and_safe_warnings`
  (`crates/farhelm-helm/src/provisioning/credential_delivery_tests.rs:1767-1771`) and
  `codex_refresh_survives_reconnect_and_mode_changes_until_replacement`
  (`crates/farhelm-helm/src/provisioning/credential_delivery_tests.rs:748`) similarly assert
  `snapshot.codex_endpoint.is_none()` without asserting `!snapshot.env.contains_key(CODEX_ENDPOINT_TOKEN_ENV)`.
- Verification: source inspection of `credential_delivery_tests.rs` lines 185-188, 534-631, 747-748, 1600-1661,
  1731-1796, `credential_delivery.rs:893-901`, and `recipe_credentials.rs:83-101`.
- Fix: Have `Fixture::snapshot()` also assert the supervisor's snapshot invariant
  (`snapshot.codex_endpoint.as_ref().is_some_and(|e| e.has_token) == snapshot.env.contains_key(CODEX_ENDPOINT_TOKEN_ENV)`),
  and replace the raw `f.claude().await.contains(...)` checks at lines 629, 1612, and 1794 with decoded
  `f.snapshot().await` assertions verifying both Claude (`ANTHROPIC_AUTH_TOKEN`) and Codex (`codex_endpoint` and
  `CODEX_ENDPOINT_TOKEN_ENV`) slots explicitly.

Reviewer metadata (g22b_codex_recipe_creds_tests/f1): suggested bucket: other (test-assertion gap in unit tests; no
direct user-facing security or data-loss impact).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g22b_codex_recipe_creds_tests/f1`.
