# Desktop startup failures can blame the embedded helm

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

When the desktop app fails to start (for example the local supervisor never connects), it may show an alert blaming the
embedded helm instead of the real problem.

## Details

Found by pre-pr-review-swarm run `20260925-0856-2b597e9-339f` (audit of Area 10, UI) as
`F24 / COR-DESKTOP-STARTUP-MISLEADING-REFUSAL`, tagged **possible**. Anchors and title:
`crates/farhelm-ui/src/desktop.rs:1402`, `crates/farhelm-ui/src/desktop.rs:1414`,
`crates/farhelm-ui/src/desktop.rs:1421` — A desktop startup failure after the embedded helm is running can be reported
as "embedded helm stopped unexpectedly"

During desktop startup, `DesktopBootstrap::start` launches the embedded helm on its own thread. Once the helm reports
ready, `start` spawns a monitor thread (line 1398) that joins the helm thread. If the helm ever stops while the
`expected_helm_shutdown` flag is still false, the monitor calls `refuse_and_exit` with "embedded helm stopped
unexpectedly" (or the helm's own error) and exits the process. On macOS that path also shows a native alert, which is
the only diagnostic a Finder-launched app gives. The flag is set only in `Drop for DesktopBootstrap`, and that value
only exists once `start()` has succeeded.

Several fallible steps come after the monitor is spawned: reading the state file, reading the token, the native
credential exchange, the 30-second wait for the local supervisor, and `RUNTIME_AUTH.set`. If any of them returns an
error through `?`, the shutdown sender is dropped. The helm's `run_embedded` treats a dropped sender as a shutdown and
returns `Ok(())`. The monitor sees the flag still false and refuses with "embedded helm stopped unexpectedly". That
races the real error, which `run()` is reporting through the same refusal path, so the user may see the wrong cause, or
on macOS two alerts. For example, a supervisor that never connects may be reported as an embedded-helm failure.

The suggested fix is to set the flag on every return path after the monitor exists, for example with a scope guard that
is disarmed on success, or to spawn the monitor only after the last fallible step.
