# A failed v0.45.0 custom-agent release shot strands a Settings draft and fails every later shot

Reviewed commit: b0608d96ee728c73678c6a4c0527395281f0f0cc

## TLDR

Maintainer tooling only. When the maintainer drafts or recaptures the v0.45.0 custom-agent release screenshot and one of
its steps fails after the agent name is typed, Settings is left open with an unsaved custom-agent draft that the capture
engine cannot close. Every shot after it in that run then fails with a misleading "layers remain after 6 Escape presses"
error: the other two v0.45.0 shots and every shot on the session-list, start-a-session, stop-restart-resume and
work-in-a-session pages. Nothing published is affected, because a failed run cannot be published.

## Details

**other.** Recorded by the trailing review of `e159ffc7..f6d8d8bb`; the tip `f6d8d8bb212be18c1f5cb6566d4fa2f92d64bacd`
still has it.

### Reviewer record (`g001/f1`)

Location: `scripts/docs-shots/release-notes.py:324`. Summary: A failed custom-agent shot leaves an unsaved Settings
draft that fails every later shot in the run.

`custom_agent_draft` (`scripts/docs-shots/release-notes.py:324-340`) fills `custom-agent-name`, then
`custom-agent-command`, and only then returns its `discard_draft` cleanup. `capture_pages`
(`scripts/docs_shots.py:420-449`) registers a cleanup only from the steps' return value, so if the second `fill` raises
(a renamed id, a bridge refusal, a timeout), no cleanup runs. The same holds if `discard_draft` itself raises partway,
since it runs in the engine's `finally`. The draft is dirty from the first `fill`
(`crates/gui/farhelm-gui/src/settings/custom_agents.rs:260-272`).

The stranded draft then defeats the per-shot `reset` (`scripts/docs_shots.py:305-324`). Escape inside the Settings
dialog, wherever focus is, goes to the dialog's dismiss hook (`crates/gui/farhelm-gui/src/layers.rs:718-724`,
`dismiss_top` at 1650-1667; the text input consumes only Enter, `components/text_input.rs:876-880`). With unsaved edits
that hook asks "Save changes before closing?" (`settings/editing.rs:572-621`, `settings/tests.rs:3100-3104`), and Escape
on that question is its Cancel, "Keep editing" (`prompt.rs:72`, `editing.rs:613-617`), which returns focus to Settings.
The reset loop alternates between the two states and raises on its seventh look. Pages are captured in sorted order
(`docs_shots.py:67`), so every page after release-notes fails too.

Fix: the module already has the pattern. The v0.42.0 `grouped_sessions` shot (`release-notes.py:48-80`) wraps its steps
after the mutation in `try`/`except` and runs its cleanup before re-raising. Do the same from the first `fill` onward,
or have `reset` click Discard when the Settings close question appears. Verify by source: follow a raise from the second
`fill` through `capture_pages` and `reset`.

`docs/docs-shots/SPEC.md` says "A failed shot writes no output and the engine continues to the next", and the
`capture_pages` docstring promises that failed checks "do not lose later shots"; this route breaks that.

Suggested bucket: other; the harm stays in maintainer tooling (wasted draft and recapture runs, misleading follow-on
errors), and the first failure is still reported.

Confidence: confirmed by source (cleanup registration, the Settings close question, Escape as Cancel on prompts). Not
observed in execution.

proposed_drop: none.

possible_cover: none. The related queue item `docs-shots-share-host-details-state.md` is about a different shot leaking
state on its success path.

Review evidence (findings log, delivery audits, dispositions and drop-check verdicts) is retained under run
`7c1cb54d-c73b-4387-bc50-5406ecc9bae4` as `g001/f1`.
