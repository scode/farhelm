# Drift harness skips PrivateConfig::finish on create/attach errors and runs copy_back even if delete_session failed

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

During a real-agent drift test for Grok or Antigravity, if session creation or initial terminal attachment fails, the
harness skips both the check that the maintainer's real hook files were untouched and the copy-back of any vendor
config/login files rewritten before the failure. Conversely, if deleting the session at teardown fails, copy-back still
runs while the agent process may still be alive and writing to the mirror.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/e2e/real_agent_capture/drift.rs:663`.

In `crates/farhelm/tests/e2e/real_agent_capture/drift.rs` lines 663-678, `client.create_session_with_extras(...).await?`
and `Terminal::attach(&sup, &client, id, false).await?` propagate errors with `?` before the teardown block at lines
758-770 that calls `private.finish()`. Yet `create_session_with_extras` runs `Supervisor::setup_vendor_hooks` (which
writes persistent hook files) and launches the vendor CLI before returning, and `Terminal::attach` explicitly handles
early CLI exits such as auth blocks (`refusal(...)` at lines 319-335). Returning early on either line bypasses
`PrivateConfig::finish()` (documented at `crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs:310-319`
as running "after the case, on every path"), so: (1) a broken setup that wrote the maintainer's real hook file before
create or attach failed will not be caught by `real_leaves_unchanged()` (and an auth failure on attach could even report
`COULD NOT RUN` while real hook files were mutated); (2) any login token the agent refreshed during startup before
`Terminal::attach` timed out or failed is dropped with the mirror. In addition, at lines 759-766, `private.finish()`
runs unconditionally even when `cleanup` (`client.delete_session(id).await`) returned `Err` and before `state`
(`farhelm_teststate::TestDir`) drops to reap hosts (`end_hosts`), contradicting the comment at line 761 ("After the
agent is gone, so nothing writes the copy any more"); `copy_back()` could therefore copy a partially written file over
the maintainer's real file, and `cleanup?` at line 766 then drops any `finished` error or notes anyway. Verification:
inspect `crates/farhelm/tests/e2e/real_agent_capture/drift.rs:662-770`,
`crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs:310-319`, and
`crates/farhelm-teststate/src/lib.rs:760-780`. Fix: run `create_session_with_extras` and `Terminal::attach` inside the
teardown-guarded scope (deleting the session if created, stopping `accepting`, and dropping `sup`/`state` to confirm
hosts ended before calling `private.finish()`), and only run `copy_back()` once session teardown/host reaping has
succeeded while still running `real_leaves_unchanged()` unconditionally on every path after `prepare`.

Reviewer metadata (g13_drift_private_hooks/f1): suggested bucket: other — harm is confined to the opt-in maintainer
drift test harness (which stays out of `highest` unless catastrophic per review_feedback_queue/AGENTS.md, "Priority
buckets").; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g13_drift_private_hooks/f1`.
