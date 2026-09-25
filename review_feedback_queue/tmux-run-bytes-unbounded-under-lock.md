# One-shot tmux commands have no timeout and resize runs under the global lock

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Every terminal on the host stops accepting typing and new attaches until the supervisor restarts.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as
`F4 / COR-RUNBYTES-TIMEOUT`, tagged **definite**. Anchors and title: `tmux.rs:2212-2222`,
`service/handlers.rs:1926-1928`, `service/handlers.rs:2201-2203` — One-shot tmux commands have no timeout, and resize
runs one while holding the supervisor-wide attachments lock

Most short tmux commands the supervisor issues go through `TmuxDriver::run` and `run_bytes`: `resize-window`,
`kill-window`, `new-window`, `set-option`, `display-message`, `list-panes`, `has-session`, `kill-session`, and
`start-server`. `run_bytes` spawns `tmux <args>` and awaits `Command::output()` with no deadline. It also does not set
`kill_on_drop`, so a cancelled caller leaves the tmux child running. The `tmux` client process blocks until the server
answers. If the server is alive but not answering (stopped with SIGSTOP, deadlocked, or deep in swap), the call never
returns.

That matters most where the call runs under the `attachments` mutex. This is a single supervisor-wide lock that
serializes attach, keyboard input, detach and teardown for every session on the host. The Attach handler
(handlers.rs:~1928) and the Resize handler (handlers.rs:~2203) both call `resize_window` while holding it. The rest of
the module is built to prevent exactly this. The docs on `CONTROL_EXCHANGE_TIMEOUT` say "a wedged tmux command must fail
the attach request instead of leaving it holding the global attachment lock forever". The control-client exchange,
`InputClient::send`, `list_client_pids_and_flags` and `run_bytes_tail` all have a deadline for that reason. `run_bytes`
is the one path left unbounded. The same gap also hangs supervisor startup (`start-server`,
`display-message #{version}`) and each connection's request loop through `resolve_terminal` → `pane_states`, though
those do not hold the global lock.

The result is that one unresponsive tmux server freezes the whole supervisor: every terminal stops accepting typing, and
new attaches and detaches queue forever with no error, until the supervisor restarts. The suggested fix is to give
`run_bytes` the same shape as `run_bytes_tail`: `kill_on_drop(true)` plus a `tokio::time::timeout` using the driver's
`exchange_timeout`, with an error that names the command. At minimum, bound the `resize_window` calls made under the
lock.
