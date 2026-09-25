# A cancelled systemd probe wedges the scope verdict at Probing

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After one badly timed disconnect during a restart, stop, delete, restart, tab operations and new sessions on that host
hang until the supervisor is restarted.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F3 / COR-PROBE-WEDGE`, tagged **possible**. Anchors and title: `scope.rs:584-616` — A cancelled systemd probe leaves
the verdict stuck at `Probing`, so every later scope operation hangs forever

`ScopeManager` is the supervisor's handle on the systemd user manager, which it uses to create, list, and kill
per-session cgroup scopes. It caches one verdict per process: `Unprobed`, `Probing`, `Usable`, or `Unusable`. The probe
runs lazily, on the first operation that needs a scope, or as the single permitted re-probe after a negative answer.

`ScopeManager::tools` sets the verdict to `Probing` under a mutex, releases the mutex, and awaits `self.probe()`. The
probe spawns `systemd-run` and `systemctl` and can take up to 15 seconds. Only after that await returns does it write
`Usable` or `Unusable` and call `notify_waiters()`. Concurrent callers that find `Probing` park on
`verdict_changed.notified()` and wait for that notification. Nothing guards the gap: if the future running the probe is
dropped mid-await (Rust async cancellation), the verdict stays `Probing` for the life of the process and no one ever
sends the notification.

There is a traced cancellable caller. Restart's pre-relaunch reap runs on a task owned by the client connection (see
F4), and the connection aborts its tasks 30 seconds after the client disconnects. That reap calls `reap_process_tree`,
which calls `available()` or `reprobe()`, and on a fresh supervisor or a re-probe that can be the call that owns the
probe.

After such a cancellation, every `available()`, `reprobe()`, `exists()`, `kill()`, `units_matching()` and
`launch_prefix()` call blocks forever. That covers every stop, delete, restart, tab open and close, and every create
that selects a scope. Each blocked operation keeps holding what it acquired first: the session's lifecycle claim (the
per-session lock that serializes stop, restart, delete and tab close), an admission permit from the supervisor's small
pool of slow-request slots, and, for Delete, the supervisor-wide working-directory mutex that creates and restarts also
wait on. The supervisor wedges until it is restarted, and no error is shown.

The premise is still open: a probe-owning future has to actually be dropped mid-probe, and the restart path is the one
route traced. The fix is to make the probe cancellation-safe. Either add a drop guard that restores the previous state
(`Unprobed` or `Unusable{..}`) and calls `notify_waiters()`, or run `probe()` on a detached `tokio::spawn` task that
always publishes a verdict.

For the user, after one badly timed disconnect during a restart, stop, delete, restart, tab operations and new sessions
on that host hang until the supervisor is restarted.
