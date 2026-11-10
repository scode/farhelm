# Codex mid-turn steering through a per-session app-server

NOTE: Historical assessment, written 2026-11-09. This is a source reading of one Codex version, not an experiment.
Nothing here was run end to end, and the idea was dropped without being built.

## The question

A Codex agent running a long goal in a Farhelm session was asked how someone outside its terminal could steer the turn
it was in the middle of. Its answer: Codex's app-server protocol has a `turn/steer` request that injects input into the
active turn, but only through an app-server that an outside client can connect to. Farhelm launches Codex with
`--no-daemon`, which keeps the app-server embedded inside the TUI process with no listening socket, so nothing outside
can reach it. What remained for that run was `codex queue` (which only delivers once the turn ends) and typing into the
TUI. The agent suggested an alternative: Farhelm could start its own `codex app-server` per session on a private unix
socket and launch the TUI with `--remote unix://<that socket>`. The question was whether that could replace
`--no-daemon` without giving up what the flag protects.

## Why `--no-daemon` is there

It came in with #2820 ("fix: isolate Codex commands from the shared server"), and SPEC_impl.md, "Codex without the
shared server", has the reasoning. Codex's shared background daemon keeps the environment of whichever client started
it, which can include another session's `FARHELM_SESSION_ID` and `FARHELM_SESSION_TOKEN`. Its commands, hooks, MCP
servers and helpers are children of the daemon rather than of the attaching TUI, and the daemon detaches with
`setsid()`. That breaks session attribution and Farhelm's stop boundary (the session's cgroup, reaping). Those
consequences were also derived from source rather than seen in a live daemon test.

## What the source says

Everything below is from OpenAI Codex
[`rust-v0.167.0`, commit `086ccc83`](https://github.com/openai/codex/tree/086ccc833bc9cf01024ad2dd6af7dbd421b6bf9a),
the version installed at the time.

The isolation would hold. The only `setsid()` outside PTY spawning is in the daemon backend
(`codex-rs/app-server-daemon/src/backend/pid_start.rs`). A plain `codex app-server --listen unix://PATH` stays in the
process tree, and therefore the cgroup, of whoever started it, and inherits that environment. Started by Farhelm inside
the session, its commands would belong to the session the same way the embedded server's do today.

Steering from a second client would be accepted. `turn_steer_inner` in
`codex-rs/app-server/src/request_processors/turn_processor.rs` looks the thread up in the process-wide thread manager,
not in anything scoped to the requesting connection, and the only direct-input refusal
(`thread_queue_processor.rs`, `ensure_direct_input_allowed`) is for subagent threads. The caller has to name the active
turn in `expectedTurnId`.

The cost is in how the TUI treats that socket. `codex-rs/tui/src/startup_orchestration.rs` only classifies a
`--remote unix://` endpoint as a local daemon when it is the shared daemon's default control socket. Any other path,
which a private per-session socket would be, is a remote workspace. In that mode:

- The TUI forwards only an allowlist of `-c` session overrides to the server (`config_request_overrides_from_config` in
  `codex-rs/tui/src/app_server_session.rs`: `allow_login_shell`, `default_permissions`, `features`, `network`,
  `permissions`, `personality`, `sandbox_workspace_write`, `shell_environment_policy`,
  `suppress_unstable_features_warning`, plus a few model settings). Farhelm's hook overrides (`hooks.*`) and its
  workspace-trust override (`projects={...trust_level=...}`) are not on it, so they would have to move to the
  `codex app-server` command line. `--dangerously-bypass-hook-trust` is forwarded per thread.
- `--add-dir` and `sandbox_workspace_write.writable_roots` overrides are refused outright with `--remote`.
- `--no-daemon` together with `--remote` is refused, which is already documented for custom commands in SPEC.md.

## Outcome

The maintainer chose to drop the idea after this assessment; this entry does not record a reason beyond the costs
below. Mechanically it looked feasible for Farhelm-composed Codex launches, but Codex arguments would have to be split
between two processes, and `{farhelm_args}` in custom commands only fills one, so
custom command launches would need a redesign. A user's own `-c` overrides in a Codex command would silently stop
applying unless the key happened to be on the allowlist. Farhelm would also own a two-process lifecycle per session:
start the server, wait for its socket, run the TUI, stop the server when the TUI exits, on every start, restart and
resume.

Several things were also never verified: whether the TUI renders input another client steered in, whether hooks
behave the same when they run from the server, and what else differs in the remote-workspace path (the app-server is
marked experimental in `codex --help`). An end-to-end test on a throwaway session was offered and not run.
