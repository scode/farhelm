# Cursor's launch-prompt drift check can never pass

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

The live check that Cursor still accepts Farhelm's launch prompt can never report a pass. Even when Cursor answers the
launch prompt correctly, the case ends as "could not run" (or as a failure when the test is run directly), so a
maintainer running the drift suite cannot tell a working Cursor launch prompt from a missing login or a broken one.

## Details

Concerns 301e8f0bd6. Locations: `crates/farhelm/tests/e2e/real_agent_capture/drift/initial_prompt.rs:58-69` (`execute`),
`:102-104` (the untracked branch of `check_fresh`), `:302-310` (`untracked_selector`).

Cursor has no hook witness in `profile()` (the Cursor arm in `drift/mod.rs` has witness None), so `await_prompt_reply`
returns `Ok(None)` and `check_fresh` calls `untracked_selector`, which returns `Ok(None)` for Cursor (the
`Cursor | Claude | ... => return Ok(None)` arm at `:306`). Back in `execute`,
`LaunchHarness::Cursor.supports_initial_prompt(true)` is true (`crates/farhelm-proto/src/launch.rs:152-158`), so
`selector.ok_or_else(...)` at `:61` always produces
`DriftError::CouldNotRun("fresh prompt passed, but this CLI did not expose a conversation selector ...")`. No path
through the Cursor case reaches `Ok(())`. Under `scripts/run-harness-drift.py` that is a COULD NOT RUN verdict (and a
nonzero exit when the case is selected explicitly, line 236); run directly through the ignored test, `run_case` panics
on `CouldNotRun`. The fresh pass shows only in the reason text.

`docs/harness-drift-tests.md:63-67` says the named second prompt is checked "where the agent also accepts a launch
prompt when it opens a named conversation", which for Cursor never happens.

Verify by reading the three locations above. Fix: the named_conversation category already has a Cursor selector source
(`cursor-agent create-chat`, `check_without_capture` in `named_conversation.rs`), so initial_prompt could seed the fresh
launch as a named start into a created chat, or skip the named sub-check for Cursor explicitly and report the fresh
pass. Test tooling only; no shipped behavior is affected.

Bucket: other (maintainer drift tooling gives an unknown verdict for one harness; no user impact). Not highest: the harm
is confined to opt-in maintainer test tooling.

Confidence: confirmed from source.

Possible cover: TODO.md has an entry "Finish Cursor launch-prompt verification", which says to "give the case a
conversation ID it can resume with" and appears to describe this gap. It sits outside TODO.md's Planned bucket, so it is
not a coverage source under the queue rules; the finding is recorded and that entry is named here as a possible cover
for triage.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g04 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
