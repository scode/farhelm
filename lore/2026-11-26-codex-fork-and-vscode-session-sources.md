# Codex forks and `vscode` conversations never become resumable

NOTE: This is a historical investigation, recorded on 2026-11-26 against Farhelm main at `215b6fff5` and OpenAI Codex
[`rust-v0.172.0`](https://github.com/openai/codex/tree/rust-v0.172.0) (commit `fc3036db`). The Codex findings come from
reading the source plus one real incident's hook log and transcript headers, not from an end-to-end test of every path.
This entry is not maintained.

## The incident

A Farhelm Codex session on a macOS remote host was launched to resume a conversation that another Codex process already
held. The machine had two app-servers running: the shared background daemon, which a terminal Codex outside Farhelm had
started, and the ChatGPT desktop app's own server. The evidence does not say which of them held the conversation.
Farhelm always launches Codex with `--no-daemon` (SPEC_impl.md, "Codex without the shared server"), so its Codex could
not attach to that process. Codex showed its read-only lock screen, "This conversation is open in another app". The
user pressed `f` to fork and submitted a turn in the fork. Restart never became available.

The session's hook log showed exactly why:

```text
written <fork-id> fork
refused invalid_request Codex reported an unsupported foreground transition <fork-id> fork
written <fork-id> -    event=Stop
acked <fork-id> -    ignored
```

Codex fired `SessionStart` with source `fork`. Farhelm's `is_foreground_source`
(`crates/farhelm-supervisor/src/agent_kind/codex.rs`) accepts only `startup`, `resume`, `clear` and `compact`, so the
selection was refused. The turn's `Stop` was then ignored, because a Codex `Stop` can only confirm a selection that a
`SessionStart` already made (SPEC.md's Codex capture rules). The fork's transcript header had the fork's own ID as
`session_id` and the string `cli` as `source`. It would have passed the exact-record check.

Its parent's header had source `vscode`, which exposed a second, independent gap. Suppose the original resume had not
been blocked. Codex would have reported `resume` with the parent's transcript, and Farhelm's `parse_record` would have
refused it as "not an attributable root conversation": it accepts only `cli` and `exec`.

## Whether accepting `fork` just works

From the 0.172.0 source, I believe it does. Here is what I checked:

- **Only top-level forks report `fork`.** `SessionStartSource::Fork` is chosen only for `InitialHistory::Forked` with a
  fork parent (`codex-rs/core/src/session/session.rs`, around line 2169). `thread/resume` with supplied history also
  uses `Forked` internally but has no parent, and it reports `resume`. Spawned subagents that fork are dispatched as
  `SubagentStart` instead (`codex-rs/core/src/hook_runtime.rs`, `run_pending_session_start_hooks`). Farhelm subscribes
  to neither of those, and its doorway already refuses reports carrying an `agent_id`. The hook schema lists `fork` as
  one of the documented sources (`codex-rs/hooks/src/schema.rs`, line 860).
- **The transcript exists when the hook runs.** `hook_transcript_path` (`codex-rs/core/src/session/mod.rs`, line 6245)
  calls `ensure_rollout_materialized` before handing the path to the hook. In the incident the rollout file was created
  two seconds before the hook line was logged.
- **A user fork stores a self-contained copy.** The TUI's fork takes the `ForkPersistence::Copied` path. The
  reference-backed fork (`fork_prepared_thread`, `codex-rs/core/src/thread_manager.rs`, around line 1930) is the one
  that points at the parent's history. It is built as `InitialHistory::Resumed`, so it reports `resume`, and its doc
  comment ties it to an inherited session ID (the subagent case). Resuming a user fork therefore does not need the
  parent, which may still be held elsewhere.
- **The fork's header matches what the hook reports.** In the incident the header's `session_id` was the fork's own ID,
  the same ID the hook reported, and its `source` was `cli`. Subagent forks differ: their header's `session_id` is the
  root session's ID, and their `source` is an object. Neither ever reaches `SessionStart`.
- **Process attribution is unchanged.** The TUI forks inside the same Codex process that Farhelm already pinned, so the
  pin judgment accepts the report. Nothing else in Farhelm's Codex admission branches on the source, apart from the
  pending-clear rule for `clear`.
- **This is the escape hatch Codex offers.** The lock screen's footer offers only `r` to retry and `f` to fork
  (`codex-rs/tui/src/chatwidget/rendering.rs`, lines 126 and 170). Anyone who resumes a held conversation in Farhelm will
  end up here.

The fix is adding `fork` to `is_foreground_source`. Both the hook doorway and admission consult that one predicate.

## What `vscode` means, and why accepting it is safe

`SessionSource::VSCode` is Codex's `#[default]` source, and the stdio app-server entry point passes it explicitly
(`codex-rs/app-server/src/lib.rs`, line 466). The parent in the incident had originator `codex-tui` and source `vscode`,
consistent with a terminal Codex attached to the shared daemon. I did not verify which sources the desktop app's own
server stamps. Codex counts `vscode` as interactive: `INTERACTIVE_SESSION_SOURCES` is `cli`, `vscode` and two custom
sources (`codex-rs/rollout/src/lib.rs`, line 89). Resume appends to the existing rollout, so the header keeps its
original source for good. As things stand, any conversation started through an app-server can never become resumable in
Farhelm, even once nothing else holds it.

The `cli`/`exec` restriction exists to keep internal and subagent records out ("Root metadata only" in
`parse_record`'s doc). `SessionSource` serializes with `rename_all = "lowercase"`
(`codex-rs/protocol/src/protocol.rs`, line 3246). The plain-string values are `cli`, `vscode`, `exec`, `mcp` and
`unknown`. `Custom`, `Internal` and `SubAgent` serialize as objects. Accepting `vscode` therefore does not weaken that
boundary, and the process-ancestry and exact-file proofs still apply.

Accepting `vscode` is still a policy change rather than a pure bug fix: Farhelm starts capturing conversations that
originated outside it. That matches the launcher's ability to resume a chosen conversation, which is why it was wanted.
`mcp` (an app-server started with `--session-source mcp`/`app-server`) was not considered in detail. The same reasoning
would likely apply to it, but no incident has shown it.
