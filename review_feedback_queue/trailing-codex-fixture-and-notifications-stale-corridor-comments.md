# Stale comments in Codex fixture and notifications describe removed executable-basename and corridor refusal rules

Reviewed commit: 3f4f312202398d9eb42fe37c9db0f58ff80f6647

## TLDR

Comments in the Codex test fixture and supervisor notification enum still describe the removed two-Codex-image corridor
refusal rule and Codex-only ancestor restriction, which can mislead future readers about how Codex process attribution
actually works.

## Details

**other.** Source inspection. Main reference `crates/farhelm-fixtures/src/fake_agent/codex_conversation.rs:462`.

Commit 3f4f312202398d9eb42fe37c9db0f58ff80f6647 removed crates/farhelm-supervisor/src/procs/codex.rs and switched Codex
to procs::positional_corridor, but left stale comments in files it touched: (1)
crates/farhelm-fixtures/src/fake_agent/codex_conversation.rs:462-464 (nested_shell_native) still says "Two Codex images
refuse this regardless of the shell between them, pinning that wrapping a nested runtime in a shell does not smuggle it
past the corridor" — under positional_corridor, that chain is attributed to the nested process and refused by the
foreground ReportPin, not by a two-Codex-image corridor rule (compare
crates/farhelm/tests/e2e/codex_identity.rs:974-976, which was updated); (2)
crates/farhelm-fixtures/src/fake_agent/codex_conversation.rs:26-28 (run) still says "while a hook launched through it
would add a second Codex process to the ancestry"; (3) crates/farhelm-supervisor/src/service/notifications.rs:84-85
(NotificationKind::CodexPinCorrected) still says "A verified Codex ancestor corrected a conversation selected by its
child. The warning preserves what a Restart before correction would have used", whereas any recorded ancestor in the
positional chain can now correct it and CODEX_PIN_CORRECTED_TEXT (lines 211-213) was updated to "could have resumed" and
"one of that process's ancestors". How to verify: inspect those lines at 1b3fe49af86979aebc2719965682a17f8c6366ac
against crates/farhelm-supervisor/src/procs/positional.rs:101-141 and
crates/farhelm-supervisor/src/service/core/vendor/codex.rs:130-160. Fix: update those comments to describe positional
attribution and ReportPin enforcement.

Reviewer metadata (g57_positional_hooks_codex/f2): suggested bucket: other (comment/doc drift only; no runtime impact);
confidence: confirmed; possible cover: FILTER.md may filter minor stale comment drift in fixtures/internal comments;
commissioner to check..

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g57_positional_hooks_codex/f2`.
