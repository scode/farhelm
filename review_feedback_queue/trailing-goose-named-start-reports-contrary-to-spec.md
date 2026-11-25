# A named Goose start into a Farhelm-created conversation still captures it, contrary to SPEC.md

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

When a user opens a Goose session on a named conversation that Farhelm itself started earlier (New with a session id, or
Restart with), Farhelm says it will not capture that conversation and that later Restart will be unavailable. Going by
the live observation recorded in the drift-test docs, Goose reports the conversation anyway, so plain Restart becomes
available again and resumes that named conversation. The launcher's warning and the spec are wrong for this case, and
the new live drift check for Goose always fails because of it.

## Details

The product behavior is pre-existing. 9209593d70 adds the Goose named-Restart drift case that asserts the spec contract;
2a309eb592 adds the docs paragraph recording the live failure. Locations:
`crates/farhelm-supervisor/src/agent_kind/goose.rs:22-73` (`inject_hooks`: a resuming launch keeps the persisted
reporter enabled when hooks are on); `crates/farhelm-proto/src/launch.rs:542-558` (`captures_named_conversation` false
for Goose); SPEC.md:4138-4145 and SPEC.md:1244-1245; `check_goose` and `recall_without_capture` in
`crates/farhelm/tests/e2e/real_agent_capture/drift/named_conversation.rs`; `docs/harness-drift-tests.md:56-61`.

SPEC.md:4140 says "Goose cannot report on this resume, so a named Restart with clears its old capture in the relaunch
transaction", and SPEC.md:4144-4145 says "Cursor, OpenCode and named Goose starts do not gain later plain Restart from
this choice"; SPEC.md:1244-1245 has the launcher tell the user "Farhelm will not capture the named conversation and
later Restart will be unavailable". But SPEC.md:4164 also says "Goose persists a credential-free named MCP reporter with
the conversation and reuses it on resume", and `inject_hooks` in `goose.rs` sets the reporter-enabled control to 1 on
every resuming launch while hooks are allowed (the `enabled` value at `:32` feeds `GOOSE_REPORTER_ENABLED` at `:56-60`
for resuming shapes too).

A named start (New with a session id, or Restart with) is compiled in `LaunchPhase::Resume`
(`crates/farhelm-supervisor/src/service/core.rs:11415-11422` for create; `host_restart.rs:264-268` for restart), and
`farhelm_args` (`goose.rs:90-129`) and `inject_hooks` set `FARHELM_GOOSE_REPORTER_ENABLED=1` on Resume whenever hooks
are allowed. A conversation created under Farhelm persists the reporter in Goose's session metadata (`goose.rs:83-88`),
so it reports itself into the new session. The design premise at `crates/farhelm-proto/src/launch.rs:539` ("Goose's
reporter is attached only to fresh starts") is therefore false for such a conversation.

The commit's own docs (`docs/harness-drift-tests.md:56-61`) record this live observation: "Goose 1.60.2 can then report
the selected conversation through the reporter it persisted when that conversation was created. The case reports this as
a failure because plain Restart becomes available again after the named Restart." The drift test's
`recall_without_capture` (reconcile, then `ensure_uncaptured`) encodes the spec, so the Goose case is a standing known
failure.

Consequences: the launcher and Restart-with text misstate what happens, and a second session can end up capturing the
same Goose conversation as the session that created it, so plain Restart in both reopens and appends to one
conversation. The user did name it, so this is not resuming a conversation that is not the session's own.

Verify by reading `goose.rs:22-73` against SPEC.md:4138-4145. Fix: either make the spec and launcher text admit that a
Goose conversation created by Farhelm reports on named starts (and set `captures_named_conversation` accordingly, or
make it conditional), or disable the persisted reporter on named Goose starts (set the reporter control to 0 when the
launch carries a named selector) so behavior matches the spec.

Bucket: other (misleading UI text and a spec mismatch; the captured identity comes from an explicit report of the
conversation actually running, so Restart resumes what the session was running). Could be highest: unclear, leaning no;
it would be highest only if the shared capture led two sessions to corrupt one Goose conversation, which is not
established.

Confidence: likely. The launch side is source-supported as described above. The remaining unverified premise is that
admission accepts that report for the named launch: no Goose- or named-specific refusal was found in the supervisor
(`captures_named_conversation` is consulted only for `clear_capture` at `core.rs:12715-12726` and in the launcher at
`crates/gui/farhelm-gui/src/launcher/view/more_options.rs:80`), and the commit author observed it live
(`docs/harness-drift-tests.md:58-61`).

Possible cover: `docs/harness-drift-tests.md:56-61` acknowledges the live failure; it may also be tracked in TODO.md
(the reviewer did not search it).

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g04 record f2; claude-opus-5-5
reviewer, source analysis only, nothing executed.
