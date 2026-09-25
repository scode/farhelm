# A supervisor death mid-sweep leaves the tree frozen and listed as running

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After a supervisor crash or restart during a stop, delete or tab close, the agent or tab can be frozen yet listed as
running until the user stops or deletes again.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F11 / COR-CRASH-FROZEN`, tagged **possible**. Anchors and title: `store.rs:333-342`, `service/sweep.rs:1115-1146`,
`crates/farhelm-helm/src/units.rs:108` — A supervisor death between SIGSTOP and SIGKILL leaves the session's tree
frozen, and startup reconciliation then lists it as running

Between its quiesce SIGSTOPs and its final SIGKILL, the kill sweep runs up to five re-enumeration passes, each of which
reads every same-user process's environment (sweep.rs:1115-1146). The only protection for that window is the in-process
`StoppedProcessGuard`, which sends SIGCONT when its Rust value is dropped. If the supervisor process dies inside the
window (crash, OOM kill, SIGKILL, or a restart that does not run destructors), no SIGCONT is ever sent.

The supervisor's systemd unit uses `KillMode=process` (crates/farhelm-helm/src/units.rs:108). That is deliberate: tmux
owns the durable sessions, so restarting the supervisor kills only the supervisor process. The consequence here is that
the frozen agent, tab shells and panes outlive the supervisor exactly as they are.

On the next start, reconciliation sees a `StopRequested` row whose pane is still alive. By design it concludes the kill
never happened and records `ConfirmRunning` (store.rs:333-342): "a live pane under a `StopRequested` row means the kill
did not land before the crash". A stopped process still counts as alive, so the session is listed as running while its
processes sit stopped forever. Delete and Close Tab write no durable intent at all, so the row or tab simply remains,
with frozen processes behind it.

A crash during an ordinary stop, delete or tab close therefore turns "stopped" into "silently frozen but shown as
running", and the terminals stop responding with no explanation.

The premise is that the supervisor dies inside the quiesce window. At startup, finish a `StopRequested` stop whose pane
is still alive by re-running the sweep, instead of clearing the intent. As a backstop, a startup pass could SIGCONT
marker-bearing processes found in the stopped state, and the STOP-to-KILL window could be shortened.

For the user, after a supervisor crash or restart during a stop, delete or tab close, the agent or tab can be frozen yet
listed as running until they stop or delete it again.
