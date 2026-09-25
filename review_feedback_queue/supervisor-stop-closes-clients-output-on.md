# A normal supervisor stop closes output clients without switching output off

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

A routine supervisor restart could end every session on the machine.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as `F16 / COR-NO-SIGTERM`,
tagged **possible**. Anchors and title: `tmux/stream.rs:1475-1501`, `tmux.rs:3467-3480`,
`crates/farhelm-helm/src/units.rs:108` — A normal supervisor stop/restart closes every output client without first
switching its output off

The tmux seam's central safety rule, stated in `OutputStream::shutdown` (stream.rs:1475-1501) and
`shutdown_output_control_client` (tmux.rs:3467-3480), is this: an output-bearing control client is switched to
`no-output` through a separate, acknowledged tmux command before its stdin is closed or it is killed. The reason is that
"even stdin EOF can make tmux 3.7b abort" when pane output is still queued for the client, which takes down the private
tmux server and every session on the host. BUGS.md records the case where the supervisor dies abruptly (SIGKILL, OOM,
segfault) as known and unfixable: "a SIGKILLed supervisor runs nothing".

A routine stop has the same shape. Nothing in the workspace installs a SIGTERM or SIGINT handler (no `tokio::signal` or
equivalent anywhere). The generated systemd unit uses `KillMode=process` (units.rs:108), so `systemctl --user stop` or
`restart`, the ordinary upgrade path, sends SIGTERM only to the supervisor. The default signal action kills the
supervisor on the spot, exactly as SIGKILL would. The kernel closes all its pipes at once, and every terminal's output
client and every session's sink sees EOF with output still on. Unlike SIGKILL, SIGTERM can be caught, so this case does
not have to share the SIGKILL exposure.

Whether it actually crashes depends on the open premise: whether the pinned tmux 3.7c still aborts on this EOF path.
BUGS.md saw the abort on distro tmux 3.6 and has not established it for 3.7b. If it does, every planned restart or
upgrade with terminals open risks ending every session on the machine. The suggested fix is to catch SIGTERM and SIGINT
in `farhelm supervisor run` and, within a bounded time budget, run the orderly forwarder shutdown
(`begin_forwarder_shutdown`) and sink shutdown before exiting. The alternative is to record in BUGS.md or SPEC_impl.md
that planned stops knowingly accept the same risk as SIGKILL.
