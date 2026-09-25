# Stopping a terminal-less session records a plain exit before killing

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Stopping such a session kills the agent but shows it as having exited on its own (or as exited while it is still running
if the sweep failed), and the label cannot be corrected later.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F7 / COR-STOP-TERMLESS`, tagged **possible**. Anchors and title: `service/handlers.rs:1097-1161`,
`service/handlers.rs:1210-1300`, `service/handlers.rs:1314-1339` — Stopping a session with no recorded terminal records
"exited on its own" before killing a possibly live agent

`handle_stop_session` chooses between two paths using only `entry.terminal`, the recorded tmux pane:

- If the pane is alive, it runs the stop lifecycle: durable intent, then the kill sweep, then an annotated "stopped by
  user" outcome.
- Otherwise it treats the agent as already dead or absent. It records a classification of how the agent ended and then
  sweeps for leftovers.

With no recorded terminal (the retained-create and reload cases described in F6), the stop always takes the second path
(handlers.rs:1097-1161). Unless a launch-failure sentinel file or a wrapper-failure shape applies, it durably commits
`Transition::ObservedExit`, which turns a `Launching` or `Running` row into a plain, unannotated exit
(handlers.rs:1210-1300). Only after that does it run the `AgentOnly` process sweep (handlers.rs:1314-1339), which kills
any live agent by its `FARHELM_AGENT_ID` environment marker. The session's durable tmux name is never probed, unlike in
Delete.

If the sweep then fails, the client gets an error, but the row already says "exited". Exit outcomes are sticky in the
store, so a later observation cannot correct them.

This inverts the stop lifecycle's own rule, intent first and outcome only after the kill is confirmed. The same handler
refuses the analogous unrecognized-pane-owner case because "recording a plain exit for a process that never exited is a
lie the user would act on".

The premise is that the agent of a terminal-less entry is actually alive. The fix is to probe the durable tmux name when
`entry.terminal` is `None`. If a live pane exists, route through `stop_live_agent` (intent, sweep, annotated outcome).
If the probe fails, refuse rather than record an exit. At minimum, run the sweep before committing any classification.

For the user, stopping such a session kills the agent but shows it as having exited on its own. If the sweep failed, it
shows the agent as exited while it is still running. Either label is permanent.
