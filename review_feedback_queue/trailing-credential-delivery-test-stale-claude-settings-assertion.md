# Backend-failure credential delivery test keeps a vacuous .claude/settings.json absence check

Reviewed commit: 67d8541dce597e3a70dc37857167ed39b6db2d68

## TLDR

The backend-failure credential delivery test still checks that the old Claude settings file was not created, instead of
checking that the failed Claude launch-credentials file or Codex config file was not written before retry and was
written after retry.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/credential_delivery_tests.rs:1644`.

In `crates/farhelm-helm/src/provisioning/credential_delivery_tests.rs:1612-1655`
(`backend_failures_leave_saved_values_and_safe_warnings`), commit `67d8541dce597e3a70dc37857167ed39b6db2d68` switched
the fixture from `Fixture::new(false)` to `Fixture::new(true)` so Codex exercises `read-home`, `read-agent-settings`,
and `write-unit`, while Claude exercises `create-directories` and `wrote-agent-credentials` against `agent-credentials`
(`f.claude_path()`). However, line 1644 still asserts `assert!(!f.home.join(".claude/settings.json").exists());`, which
is now vacuously true on a fresh host because Claude delivery never writes `.claude/settings.json`. As a result: (1) for
`create-directories` and `wrote-agent-credentials`, the test never asserts `!f.claude_path().exists()` before clearing
the planted failure; (2) for `read-home`, `read-agent-settings`, and `write-unit`, Claude succeeds before Codex fails,
so neither `!f.home.join(".codex/config.toml").exists()` before retry nor `config.toml` containing `SECOND` after retry
(line 1653 only checks `f.claude().await.contains(SECOND)`, which was already written before the failure was cleared) is
asserted. Fix direction: assert `!f.claude_path().exists()` on the Claude-failing operations (`create-directories`,
`wrote-agent-credentials`), assert `!f.home.join(".codex/config.toml").exists()` on the Codex-failing operations
(`read-home`, `read-agent-settings`, `write-unit`), and assert after the retry that both `f.claude()` and
`~/.codex/config.toml` contain `SECOND`.

Reviewer metadata (g22-recipe-creds-specs-tests/f1): suggested bucket: other (could not be in highest; harm is confined
to a unit test assertion gap); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g22-recipe-creds-specs-tests/f1`.
