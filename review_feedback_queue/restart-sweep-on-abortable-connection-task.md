# Restart's stop-and-sweep runs on an abortable connection task

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

If the client disconnects during a slow restart, the agent can be left half-killed or still running, not restarted,
shown as "stopping", and its eventual natural exit labelled a user stop.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F4 / COR-RESTART-ABORT`, tagged **possible**. Anchors and title: `service/core.rs:9980-10027`,
`service/handlers.rs:2229-2254`, `service/connection.rs:674-695` — Restart's stop-and-sweep phase runs on a
connection-owned task that is aborted after a client disconnect

The supervisor serves each client connection with a set of spawned handler tasks. When a connection closes,
`handle_connection` (connection.rs:674-695) gives those tasks `HANDLER_SHUTDOWN_TIMEOUT` (30 seconds) to finish and then
aborts them. Stop and Delete deliberately avoid that: they run their kill sweep on a supervisor-owned `tokio::spawn`
task, so that, in the Stop handler's own words, "disconnect cleanup cannot strand a SIGSTOPped tree"
(handlers.rs:1045-1049). The sweep's escalation is SIGTERM, a grace period, SIGSTOP to freeze everything, SIGKILL, and
confirmation.

Restart does not follow that pattern. `handle_restart_session` (handlers.rs:2229-2254) runs `restart_session` through
`spawn_admitted`, which places it in the connection's abortable task set. Only the final `relaunch` step is moved onto a
supervisor-owned task (core.rs:10027 onward). Everything before it runs on the abortable task: waiting for directory
admission and the session's lifecycle claim (the per-session lock), capturing conversation state, writing the durable
`StopRequested` intent, `stop_live_agent`'s scope kill and process sweep (up to several 5-second graces), and the
leftover-descendant reap (core.rs:9980-10027).

If the abort lands inside that phase:

- `StopRequested` stays recorded and no `StopCompleted` follows.
- The agent may have received SIGTERM but never SIGKILL. The sweep's SIGCONT guard un-freezes anything it had stopped,
  so nothing stays frozen, but nothing is finished either.
- The lifecycle claim is released mid-operation, so another stop, restart or delete can start while signals are still in
  flight.
- No relaunch happens.

An agent that ignores SIGTERM keeps running under a `StopRequested` row. When it later exits on its own, the store turns
an observed exit from `StopRequested` into "exited, stopped by user" (see F5). The same cancellation can also trigger F3
if it lands inside a systemd probe.

The premise is that the pre-relaunch phase is still running about 30 seconds after the disconnect. That can happen if
the restart queued behind a Delete holding the directory mutex or behind a held lifecycle claim, or on a slow user
manager.

Run the whole pre-relaunch phase (liveness recheck, `stop_live_agent`, leftover reap) on the supervisor-owned task that
already carries the lifecycle claim for `relaunch`, and have the connection task only await the reply, as Stop and
Delete do.

For the user, if the client disconnects during a slow restart, the agent can be left half-killed or still running, not
restarted, shown as "stopping", and its eventual natural exit labelled a user stop.
