# Harness drift Turn-2 hook check passes on quiet no-op Stop acknowledgments for Codex and Antigravity

Reviewed commit: 381a6f5b8620d310686521864fce8860fd3b1a39

## TLDR

The live harness drift test for Codex and Antigravity can report a pass after Restart even when the resumed agent's hook
never verified its transcript or established a new process pin, because a silently ignored end-of-turn Stop callback
still logs an acknowledgment carrying the conversation ID inherited from Turn 1.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/e2e/real_agent_capture/drift.rs:547-561`.

In `crates/farhelm/tests/e2e/real_agent_capture/drift.rs:547-561`, `TurnExpectation::saw_report` uses
`HookWitness::MatchingIdentity` to guard against callbacks that the supervisor acknowledges without updating capture,
checking that `snapshot.resume_argv == original_argv` and that a post-baseline `acked` line in `hook-log/<id>.log` names
`reported_runtime_identity(conversation)`. However, Restart preserves Turn 1's `captured_conversation` and `resume_argv`
while clearing `report_pin`. Two admission paths return `Ok(())` without verifying or committing while still causing
`crates/farhelm-supervisor/src/service/report_files.rs:245-248` to append `<ts> acked <id> -` with that same inherited
ID:

1. Codex (`crates/farhelm-supervisor/src/service/core/vendor/codex.rs:183-200`): when `SessionStart` fails or does not
   fire on resume, a Turn-2 `Stop` callback (`enrichment == true`, `correction == false` because Restart cleared
   `report_pin`) sees `previous.resumable == true` and returns `Ok(())` at line 199 before calling `locator.verify()`,
   before pinning the resumed process, and before writing to the store. `report_files.rs:247` still logs `acked <S1> -`,
   which matches the inherited `runtime_session_id` and `resume_argv`, so `drift_codex` passes without any Turn-2
   verification or admission.
2. Antigravity (`crates/farhelm-supervisor/src/service/core/vendor/positional.rs:189-203`): if Turn 2's `SessionStart`
   fails or is missing and Turn 2's `Stop` fails `verifies_main_conversation` (e.g., transcript format drift on resumed
   sessions), line 196 returns `Ok(())` without pinning or committing. `report_files.rs:247` still logs `acked <C1> -`
   matching Turn 1's inherited ID and `resume_argv`, so `drift_antigravity` passes even though transcript verification
   rejected the resumed callback. Fix direction: Because Restart clears `report_pin` for all four `MatchingIdentity`
   harnesses (Claude, Codex, Antigravity, Muse) and only a committed report writes a new pin, requiring the resumed
   generation to have a populated `report_pin` (e.g. via a test snapshot field or checking `sup` state) — and/or
   distinguishing committed `acked` entries from no-op `Stop` acknowledgments — proves that Turn 2's report actually
   passed verification and admission.

Reviewer metadata (g01-tooling-deflake-drift-fixtures/f1): suggested bucket: other (could not be in highest: confined to
opt-in maintainer drift tests); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g01-tooling-deflake-drift-fixtures/f1`.
