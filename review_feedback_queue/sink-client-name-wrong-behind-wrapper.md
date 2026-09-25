# Sink shutdown targets the wrapper's pid behind a tmux wrapper

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

With such a wrapper, once every viewer of a session disconnects, none of its terminals can be opened again until the
supervisor restarts.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as
`F8 / COR-WRAPPER-SINK-NAME`, tagged **possible**. Anchors and title: `tmux/sink.rs:67-73`, `tmux/sink.rs:273` — The
session sink's shutdown has the same `client-<pid>` assumption

The per-session sink from F5 derives its tmux client name the same way:
`client_target: child.id().map(|pid| format!("client-{pid}"))` (sink.rs:67-73), used by the no-output handshake at
sink.rs:273. Behind a non-`exec` wrapper this name never exists, so `SessionSink::shutdown` fails on every attempt. When
the last viewer of a session leaves, the sink's registry entry becomes `Reaping`, and `shutdown_session_sink_until_safe`
loops forever. The dead-sink reap loop in `run_session_sink` has the same problem. The entry never clears, so
`ensure_session_sink` waits and then fails every later attach to any terminal of that session ("the previous session
sink … did not finish shutting down").

This is a separate code path from F7, and fixing the output stream alone would not cover it. Under F7 one terminal
becomes unattachable; here the whole session does once its last viewer disconnects, until the supervisor restarts. The
open premise is the same as F7: a wrapper that does not `exec`. The fix is the same helper as F7: learn the real client
name from tmux during the sink's attach handshake, or refuse the configuration at open.
