# Restart relaunches over an unconfirmed-empty previous scope

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

With systemd present, a restart can report success and start a fresh agent while a background server from the previous
run is still alive in the old scope, causing port or lock conflicts.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F24 / SEC-RESTART-WARN`, tagged **possible**. Anchors and title: `service/core.rs:10017-10025`,
`service/sweep.rs:1737-1746`, `service/sweep.rs:1340-1356` — Restart relaunches even when the previous run's cgroup
could not be confirmed empty

`reap_process_tree`, the single entry point for every stop, delete and restart kill, takes a `ScopeKillFailure` policy
for what to do when the systemd scope kill fails but the portable process sweep finds nothing (sweep.rs:1340-1356).
Under `Warn` it logs and returns success. Under `Refuse` it returns an error. `kill_scope` reports failure only in two
cases: the unit is still loaded 2 seconds after SIGKILL ("its cgroup may still hold processes"), or the manager stopped
answering. Delete uses `Refuse` for exactly that reason. A still-loaded scope may hold the one kind of process the sweep
cannot see, a double-forked daemon with a scrubbed environment.

Both of restart's pre-launch reaps use `Warn`. One is `stop_live_agent`, used when the agent is alive
(sweep.rs:1737-1746). The other is the leftover reap when the agent had already exited (core.rs:10017-10025). So with
the old launch scope still loaded, restart logs a warning and launches the new agent. The survivors it leaves behind are
exactly what the scope exists to catch.

SPEC says "Restart reaps any leftover descendants of the prior run before relaunching — never alongside them." The new
agent then runs next to a daemon from the old run that may hold ports, locks, or the working copy. The rationale for
never letting a scope failure fail a stop fits Stop, where nothing new launches, but not obviously Restart.

The open premise is whether restart is meant to share Stop's `Warn` policy. The fix: use `ScopeKillFailure::Refuse` for
both pre-launch reaps (add a policy argument to `stop_live_agent`, and pass `Refuse` at core.rs:10023), and refuse the
restart with the reason instead of relaunching. Stop keeps `Warn`.

For the user, with systemd present, a restart can report success and start a fresh agent while a background server from
the previous run is still alive in the old scope, causing port or lock conflicts.

Restater note: The review said nothing states restart's intended policy, but the `ScopeKillFailure` doc comment
(sweep.rs:223-229) does: "STOP and restart retain the process-tree sweep's original guarantee even when a manager
operation fails". Restart's `Warn` is therefore documented as intended, though without a restart-specific reason. The
real question is whether that documented choice conflicts with SPEC's "never alongside them".
