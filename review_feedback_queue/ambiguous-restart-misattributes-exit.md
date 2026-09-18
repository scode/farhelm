# An ambiguously failed restart durably misattributes the old run's death

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

When a restart fails ambiguously (the supervisor cannot tell whether the new agent started), the session is reported as
normally exited with the old run's exit code instead of "unknown" — and the answer flips between "exited" and "unknown"
every time the supervisor restarts.

## Details

Source: pre-pr-review-swarm, area supervisor-lifecycle, 2026-09-17. Confidence: definite (correctness-systems p1).
Coordinator confirmed the republish shape and the live/reload split.

On ambiguous failure, `relaunch` recovery republishes the entry with the previous run's terminal
(`entry.terminal.clone()`, core.rs:8121), outcome `Launching` (core.rs:8083), and the new generation, while the durable
row has its pane cleared (`begin_relaunch` sets `pane = ''`, store.rs:3086). On the next list/ticker pass the live
classifier sees the old dead pane under matching ids and (a) reports `Exited` via `session_status`'s dead-pane arm
(status.rs:203-217), and (b) commits `ObservedExit` via `observation()` (status.rs:497-517), since the generation
matches and `Transition::apply` accepts `ObservedExit` against `Launching` (store.rs:526-528). The reload path
explicitly refuses to do this (stale-pane rule, core.rs:5031), so the identical row reports `Unknown` after a supervisor
restart — the status flaps across restarts, and the module's "read half and commit half must agree" contract is
satisfied on the wrong answer.

Suggested fix: treat `Launching` plus a dead pane as `Unknown` with no transition in both `session_status` and
`observation()`, extending the "a launch whose side effects were never found has not established that it ran" rule from
the no-pane case to the dead-pane case. The sentinel check in `observe_entry` runs first, so genuine exec failures of
the new generation still surface as `Error`. This matches reload even where the respawn did take and the new agent
already exited (reload reports `Unknown` there too), removing the split rather than regressing. See also the reload-path
sibling `reload-adopts-stale-pane.md`.
