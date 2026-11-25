# Capture-timing drift check cannot tell "at the first prompt" from "after the first reply"

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

Farhelm tells users when each agent's conversation becomes resumable (as it starts, when you send the first prompt, or
after the agent's first reply), and it warns about a silent hook only for agents whose report is due by the first
prompt. The new live drift check meant to catch an agent update that starts reporting at the wrong time only checks that
Resume is not offered before the first prompt and is offered after the turn. If Codex, Muse, Grok or Antigravity started
reporting only after the reply, or Pi or OMP started reporting at prompt time before the conversation is saved, the
check would still pass, so the maintainer would not learn that the promised timing (and the silent-hook warning built on
it) no longer matches the agent.

## Details

Concerns 6ac58ec6af (category added) and ea4ba34b98 (refactor of the same file). Main reference
`crates/farhelm/tests/e2e/real_agent_capture/drift/capture_timing_hooks.rs:362-401` (`fn enabled`).

`fn enabled` branches on `harness.agent_kind().restart_readiness()`. AtStart and ShortlyAfterStart (Claude, Goose) get
`startup_capture`, a real positive timing check. FirstPrompt (Codex, Muse), FirstPromptWithHooks (Grok, Antigravity) and
FirstReply (Pi, OMP) share one arm: `early_capture` polls for 3 s at the idle composer for a premature Resume, then
`submit()` and `wait_for_turn()` wait until both a rendered reply and an acked resumable capture exist
(`wait_for_turn_matching` in `drift/mod.rs`), and `check_turn_reports` looks for the post-submit hook-log lines with no
ordering against the reply. Nothing distinguishes FirstPrompt from FirstReply: for FirstPrompt agents nothing requires
the report before (or independent of) the reply, and for FirstReply agents nothing checks that Resume stays unavailable
between Enter and the reply.

The distinction is product behavior, not wording. `RestartReadiness::due_by_first_prompt`
(`crates/farhelm-proto/src/lib.rs:1579`) gates the supervisor's silent-hook notification (`notify_hook_silent`,
`crates/farhelm-supervisor/src/service/capture.rs:131-143`), and `early_capture`'s own doc says "A pending locator can
be stored before the agent has persisted any conversation to reopen", which is exactly the harm a FirstReply agent
reporting at prompt time would cause, but `early_capture` runs only before the prompt. The `capture_timing_hooks`
paragraph in `docs/harness-drift-tests.md`, which says the category "checks whether Restart becomes available before or
after the first text turn", describes the binary check honestly; the commit message and the docstring of
`capture_timing_covers_every_tracked_builtin_agent` ("Adding a timing promise to the launcher also needs a live case
that exercises it") claim more.

Verify by reading `fn enabled`: the FirstReply and FirstPrompt variants share one match arm with identical handling. Fix
sketch: after Enter, for FirstReply agents poll and fail if Resume (or an acked capture) appears before the reply is
rendered; for FirstPrompt agents, require the matching post-submit report (written line or ack) no later than the Stop
report, or within a bound that does not depend on the reply, so a report that arrives only at turn end is flagged. Fast
replies make this racy and need care; a weaker but useful form checks the order of hook-log lines (the startup,
UserPromptSubmit or SessionStart line must precede `event=Stop` for FirstPrompt agents).

Bucket: other. A gap in maintainer test tooling; the product harm it would miss (a wrong silent-hook warning, Resume
offered before a Pi or OMP conversation is saved) occurs only if a vendor update drifts. Not highest: harm is confined
to maintainer tooling and matters only after a vendor drift.

Confidence: confirmed from source (`fn enabled` and `wait_for_turn_matching`).

Possible cover: the capture_timing_hooks paragraph in `docs/harness-drift-tests.md` describes only a
before-or-after-the-turn check, which could be read as the intended scope.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g05 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
