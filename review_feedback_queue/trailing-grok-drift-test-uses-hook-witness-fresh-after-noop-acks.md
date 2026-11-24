# Grok drift test still uses HookWitness::Fresh after Grok admission gained quiet no-op acks

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

The opt-in real-agent drift test for Grok can now accept a quietly ignored turn callback on the resumed turn as proof
that the restarted Grok session reported its conversation, instead of verifying that the acknowledged report actually
matched the resumed conversation.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/e2e/real_agent_capture/drift.rs:119-127`.

- In `crates/farhelm/tests/e2e/real_agent_capture/drift.rs:52-58, 504-520`, agents whose report admission can
  acknowledge a callback (`Ok(())`, logged as `acked` by `report_files.rs:246-248`) without applying it use
  `HookWitness::MatchingIdentity` (Claude, Codex, Antigravity, Muse) so Turn 2 after Restart cannot pass on a stale or
  subordinate ack while `resume_argv` still holds Turn 1's inherited capture.
- Before `b14f3355cb2e513220e8251112c8316670aad9e7`, `report_grok_conversation` never returned `Ok(())` without writing,
  so `HookWitness::Fresh` at `drift.rs:124` was sound.
- `b14f3355cb2e513220e8251112c8316670aad9e7` added `crates/farhelm-supervisor/src/service/core/vendor/grok.rs:180-183`
  (`if event != "SessionStart" && decision != PinDecision::Accept { return Ok(()); }`), which now logs `acked` without
  mutating capture for rival or ancestor `UserPromptSubmit`/`Stop` callbacks. `drift.rs:124` was left on
  `HookWitness::Fresh`, so `saw_report` accepts any new `acked` line on Turn 2.
- Note also that `reported_runtime_identity` (`crates/farhelm/tests/e2e/real_agent_capture.rs:401-411`) only unwraps
  `codex:` locators (not `grok:` locators, where the incoming `acked` log entry carries
  `selected_at: null, resumable: false` while the stored snapshot carries `selected_at: {...}, resumable: true`), so
  switching Grok to `HookWitness::MatchingIdentity` also requires matching `session_id` across `grok:` locators (and
  handling spaces in any logged `session_file` path).
- Verification: inspect `crates/farhelm-supervisor/src/service/core/vendor/grok.rs:180-183`,
  `crates/farhelm/tests/e2e/real_agent_capture/drift.rs:52-58, 119-127, 504-529`, and
  `crates/farhelm/tests/e2e/real_agent_capture.rs:401-411` at `1b3fe49af86979aebc2719965682a17f8c6366ac`.
- Fix: switch Grok in `drift.rs:124` to `HookWitness::MatchingIdentity` and extract `session_id` from `grok:` locators
  when comparing acknowledged reports against the snapshot's conversation.

Reviewer metadata (g56_positional_hooks_grok/f3): suggested bucket: other (opt-in maintainer drift-test witness
weakness; no user-facing runtime impact).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g56_positional_hooks_grok/f3`.
