# Startup reap misnames terminal-backed control clients

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A supervisor start can fail with "stale control clients survived" if such a client is attached.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as
`F9 / COR-REAP-CLIENT-NAME`, tagged **possible**. Anchors and title: `tmux.rs:2050-2051` — The startup reap of leftover
control clients addresses them by `client-<pid>`, which does not match terminal-backed clients

When the supervisor starts, `reap_stale_control_clients` cleans up control-mode clients that a dead predecessor may have
left attached to the private server; such clients can stay wedged forever. It lists clients as
`#{client_pid} [#{client_flags}]`. For each control-mode client it runs the no-output handshake against `client-<pid>`
(tmux.rs:2050-2051) and then kills the pid. A client whose handshake fails is recorded as skipped, not killed. A
verification loop then waits up to 5 s for zero control-mode clients and otherwise fails startup with "stale control
clients survived the startup reap … refusing to start beside them".

tmux uses the `client-<pid>` name only for clients that have no terminal. A control client started from a real terminal
(for example iTerm2's `tmux -CC` integration) is named after its tty, such as `/dev/ttys003`. For such a client the
handshake target does not resolve, so the client is skipped, but the verification loop still counts it and the
supervisor refuses to start. The docstring promises the opposite: a control client on the private socket "WILL be reaped
at the next supervisor start". The error names no client and suggests no fix. The premise is unsupported usage: someone
attaching a terminal-backed control client to Farhelm's private tmux socket. The reviewer confirmed the naming rule on
3.7c but did not test the failure end to end. The suggested fix is to add `#{client_name}` to the listing format and
target that name rather than building `client-<pid>`, and to include the name in the skipped-client message so an
operator can detach it.
