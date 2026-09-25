# Delete kills scopes serially under the global directory lock

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

While the systemd user manager is unresponsive, one Delete can make creating, restarting or deleting any session hang
for minutes.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F21 / COR-SCOPE-SERIAL`, tagged **possible**. Anchors and title: `service/sweep.rs:1333-1337`,
`service/sweep.rs:1467-1560`, `scope.rs:716-910`, `service/handlers.rs:1424-1443` — Delete kills scope units serially
with multi-second bounds each while holding the supervisor-wide directory lock

Delete kills every systemd scope unit it has named, one after another (sweep.rs:1333-1337): every tab scope, every
launch-generation scope, and the row-recorded one. Each call to `systemctl` is bounded by a 5-second `SYSTEMCTL_TIMEOUT`
(scope.rs:716-910). `kill_scope` (sweep.rs:1467-1560) makes several such calls per unit, and none of them stops early:

- an existence check (on timeout it proceeds as "cannot tell")
- SIGTERM
- SIGHUP, for tab units
- a grace loop of existence checks
- SIGKILL
- a confirmation loop

Against a manager that accepts connections but does not answer, each call waits out its full timeout, so a unit costs
roughly half a minute. Nothing short-circuits once the manager has proven unresponsive, and units are not killed in
parallel.

Throughout, Delete holds its admission permit, the session's lifecycle claim, and the supervisor-wide
`working_copy_operations` mutex (handlers.rs:1424-1443), so every create and restart on the host queues behind it. At
the end the result is still a refusal (`ScopeKillFailure::Refuse`), and the user has to retry.

The per-call timeout was meant to keep a wedged manager from hanging teardown. Serial bounded calls multiply by the
number of units inside a supervisor-wide lock; ten scopes come to several minutes.

The premise is a user manager that stops answering. After the first manager-query timeout in a teardown, fail the
remaining scope work fast: record the remaining units as unconfirmed and skip their escalation. Alternatively, put one
overall deadline on the scope phase, or kill units concurrently with a small bound. Either way, still run the portable
sweep and still refuse under `Refuse`.

For the user, while the systemd user manager is unresponsive, one Delete can make creating, restarting or deleting any
session hang for minutes.

Restater note: A manager that is unresponsive from the start does not reach the serial-kill phase. Delete first lists
the session's scopes with `units_matching`. When that times out while the cached verdict still says a manager is
available, Delete refuses immediately with `TabScopeEnumeration` (teardown.rs:283-290), after one 5-second timeout. The
multi-minute stall therefore needs a manager that answers the `list-units` calls and then stops answering the per-unit
show and kill calls. The per-unit cost is also nearer 25 to 35 seconds than the 35 to 40 seconds originally estimated,
because each grace and confirmation loop ends after its first timed-out check.
