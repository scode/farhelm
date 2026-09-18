# Archive discards a stopped agent's exit code and can misattribute a natural exit

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Archived sessions always show a stopped agent with no exit code, and an agent that happened to exit on its own just
before the archive is misreported as deliberately stopped by the user.

## Details

Source: pre-pr-review-swarm, area supervisor-teardown, 2026-09-17. Confidence: possible. Same class as the queued
`ambiguous-restart-misattributes-exit.md`.

When the pane probe found a live agent (`stopped_live_agent`), archive synthesizes
`LastOutcome::Exited {
exit_code: None, annotation: Some(STOP_ANNOTATION) }` without ever asking tmux for the code
(teardown.rs:403-407). Two consequences: (1) every archived live agent loses its exit code, even though `remain-on-exit`
keeps the dead pane queryable until `kill_session` runs — `stop_live_agent` (sweep.rs:1748) re-queries via
`dead_pane_exit_code` after the sweep for exactly this reason, so stop preserves codes archive drops; (2) TOCTOU
misattribution: an agent that exits naturally (say, code 5) between the pane probe and the first sweep signal is
recorded as a deliberate user stop with no code. This cuts against the module's own stated rationale for the dead-pane
case — "treating that case as a fresh user stop would discard an exit code … that the supervisor already knows"
(teardown.rs:61-64). Impact is display fidelity on archived sessions (status/annotation/exit code), not recoverability.

Suggested fix: mirror `stop_live_agent` — query `dead_pane_exit_code` after `reap_process_tree` and before
`kill_session`, and use the returned code when present.
