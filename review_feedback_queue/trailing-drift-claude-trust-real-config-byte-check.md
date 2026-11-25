# Claude trust drift case fails whenever another Claude session rewrites the real config

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

The live drift check for Claude workspace trust compares the maintainer's real Claude configuration byte for byte before
and after the case. Claude rewrites that file itself during ordinary use, so when any other Claude session is active on
the machine the case can fail with "the real ~/.claude.json changed while a drift case ran", a message that reads as if
the test or Farhelm wrote the real config. The Claude trust verdict becomes noise, and the maintainer may chase a leak
that does not exist. How often a running Claude session rewrites the file within one case has not been measured.

## Details

Concerns 547d11f042, which adds the copied configs to `real_leaves_unchanged`; 21f0fcadd8 leaves it unchanged. Main
reference `crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs:255-271`
(`Mirror::real_leaves_unchanged`; the new loop over `copied` starts at `:264`). It is called from
`PrivateConfig::check_hook_setup` (`private_config.rs:569-587`) via `DriftEnv::with_session` (`drift/mod.rs:752-755`)
right after the session is created, and again from `PrivateConfig::finish` (`private_config.rs:595-623`) via
`with_drift_supervisor_using` (`drift/mod.rs:874-880`).

For hook mirrors, `real_leaves_unchanged` guards files only Farhelm's setup writes, so byte equality is a sound "we did
not touch it" check. 547d11f042 extends the same check to `copied`: the real `~/.claude.json`,
`~/.claude-custom-oauth.json`, `~/.claude/.config.json` and the `CLAUDE_CONFIG_DIR` equivalents, which every running
Claude process rewrites (startup counters, per-project stats on exit, cached feature data).
`read_optional(path)? == *before` fails on any byte change. The result reaches the verdict: `with_session` returns the
error before the case's checks, and `finish`'s error is folded in by `after_cleanup`, turning a passing case into a
failure (`drift/mod.rs:883-899`; anyhow errors convert to `DriftError::Failed` at `drift/mod.rs:53-57`). The runner runs
cases one at a time (`scripts/run-harness-drift.py:130-135`), so the noise comes from the user's own concurrent
sessions, not other drift cases.

`docs/harness-drift-tests.md` (the last sentence of the Claude paragraph under "Hook setup in a private copy...") says:
"The case fails if your real files changed while it ran, which a Claude session of your own running at the same time can
cause." That acknowledges the behavior, but AGENTS.md, "Sharing the machine with other agents", says several agents run
on the development machine at once as the same user, so concurrent Claude sessions are the common case, not an edge.

The check's real purpose (prove Farhelm did not write trust into the real config) is narrower: what matters is that the
real file did not gain `projects[<canonical scratch workspace>]` (or any `hasTrustDialogAccepted` for that key). Verify
by reading the second loop of `real_leaves_unchanged` and the two call sites above; nothing filters to the trust key.
Fix: for `copied` files, compare only that the real document's `projects[<work key>]` is unchanged or absent (or keep
byte equality but report a mismatch that lacks the workspace key as a `drift:` note rather than a failure). The rest of
the case (`FailOnPrompt` plus `check_claude_trust`) already catches the production writer selecting the real file, since
the private copy would then lack trust and Claude would ask.

Bucket: other (false failures in maintainer tooling; no user data at risk). Not highest: the effect is a spurious red
verdict in an opt-in live test, and the user's files are untouched.

Confidence: likely. Source shows any byte change fails the case; the unverified premise is how often a running Claude
Code session rewrites `~/.claude.json` within one case's window (tens of seconds to a few minutes). Not observed by
execution.

Possible cover: the `docs/harness-drift-tests.md` sentence quoted above. The reviewer named it as a possible drop basis
but did not propose dropping: the doc is not the spec, and it understates how routine concurrent Claude sessions are on
the development machine.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g06 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
