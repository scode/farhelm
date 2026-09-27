# One-shot tmux commands other than resize have no timeout

Reviewed commit: 7cc06814956a1e9b6ec41f29e2e57ec57b5d2178

## TLDR

If the private tmux server stops answering, supervisor startup and some per-connection requests can hang forever instead
of failing with an error.

## Details

Narrowed from the original finding (pre-pr-review-swarm run `20260925-0414-2b597e9-0497`, `F4 / COR-RUNBYTES-TIMEOUT`).
The part that froze every terminal on the host is fixed: `resize_window`, which attach and resize call while holding the
supervisor-wide `attachments` mutex, now goes through `TmuxDriver::run_bytes_within` with the driver's
`exchange_timeout` and `kill_on_drop`.

What remains: every other `TmuxDriver::run` / `run_bytes` caller still awaits `Command::output()` with no deadline and
without `kill_on_drop`, so a cancelled caller leaves the tmux child running and an unresponsive server blocks the caller
forever. That covers supervisor startup (`start-server`, `display-message #{version}`), each connection's request loop
through `resolve_terminal` → `pane_states` (`list-panes`), and mutating commands such as `new-window`, `kill-window`,
`kill-session` and `set-option`. None of these holds the global attachments lock.

A blanket timeout in `run_bytes` is the obvious next step, but it is not a mechanical change: a timeout on a mutating
command leaves its outcome unknown (the window may or may not exist), so each mutating caller needs to decide how it
treats that. Read-only callers (`list-panes`, `display-message`, `has-session`) can take the bounded form directly.
