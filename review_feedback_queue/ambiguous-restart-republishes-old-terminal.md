# An ambiguous restart failure republishes the old terminal

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After a restart that fails partway, a running agent may be unopenable, and pressing Restart again can kill it without
asking.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F10 / COR-RESTART-REPUBLISHES-OLD-TERMINAL`, tagged **possible**. Anchors and title: `service/core.rs:10272-10307`,
`service/core.rs:10686-10711`, `service/core.rs:12415-12440` — An ambiguous restart failure republishes the pre-restart
terminal instead of the new one

Each session's agent runs in a tmux pane. The in-memory session entry records that pane as its `terminal`, which attach,
input and liveness checks use. When a restart finds the old pane gone, `relaunch_into_terminal` does not reuse it. It
creates a brand-new tmux session under the session's tmux name through `spawn_agent`, which returns a new pane id.

If a tmux step fails after that, `unwind_failed_relaunch` (core.rs:10686-10711) asks tmux whether the session exists. If
tmux says yes, or the probe fails, the failure is classed as ambiguous: the new agent may well be running. The recovery
code at core.rs:10272-10307 then republishes the entry with `entry.terminal.clone()`, which is the pre-restart terminal:
either `None` or the old, gone pane. The new pane is never recorded. When the failing step was `mark_window` (tagging
the new window as the agent's, core.rs:12415-12427), `spawn_agent` actually had the new pane id in hand and dropped it,
because the `SpawnFailure::Tmux` error variant has no field for it.

This has two consequences. First, opening the session targets a pane that no longer exists, so the possibly-running new
agent cannot be reached until a supervisor restart rediscovers it. Second, a later Restart probes the stored pane in
`restart_session` (core.rs:9916-9962). A stale pane reads as `Gone`, so the `stop_if_running` confirmation ("this agent
is still running, confirm stopping it") is skipped, and the leftover-process reap runs. The reap targets the session's
process marker and the new generation's scope, both of which cover the new live agent. The new agent can therefore be
killed without the consent prompt.

This depends on a tmux failure after the new session exists: a `mark_window` failure, or a lost `new-session` reply. The
minimal fix is to republish with `terminal: None` in the ambiguous fresh-terminal branch, so nothing points at a dead
pane. The better fix is to carry the pane in `SpawnFailure::Tmux` and publish `Terminal { tmux_name, pane }` whenever
tmux returned one. What the user sees: after a restart that fails partway, a running agent may not be openable, and
pressing Restart again can kill it without asking.

Restater note: Even with the `terminal: None` fallback, a later Restart would still skip the consent prompt, because
`restart_session` treats a terminal-less entry as not running (`pane_state` is `None`). The Area 1 swarm already
recorded that "restart of a terminal-less entry kills without consent". Only carrying the real pane closes the consent
gap.
