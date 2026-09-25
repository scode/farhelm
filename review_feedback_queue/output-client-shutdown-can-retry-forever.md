# The output client can get stuck in the same shutdown retry

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Potentially a terminal that cannot be reopened until the supervisor restarts.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as `F6 / COR-OUTPUT-STUCK`,
tagged **possible**. Anchors and title: `tmux/stream.rs:1368-1395`, `tmux/stream.rs:1520-1540` — The per-terminal output
client can get stuck in the same endless shutdown retry

Besides the per-session sink, each attached terminal has its own control-mode client that carries that terminal's output
to the browser (`OutputStream`). It shuts down by the same rule as the sink in F5: `disable_output_before_shutdown`
(stream.rs:1368-1395) must get an acknowledged `refresh-client … -f no-output` before stdin is closed and stdout
drained. If that fails while the process is alive, the stream is handed to `OutputReaper::run` (stream.rs:1520-1540),
which retries forever and never drains.

The same livelock as F5 follows if the tmux session disappears while this client's pipe is not being read. The forwarder
deliberately stops reading when the browser pauses that terminal's output, and during a stall before the stall detach.
Once the session is gone, the `refresh-client` can never succeed, and the client cannot exit because its pipe is full.
The terminal's per-terminal cleanup barrier then stays in `Reaping` forever, so every later attach to that terminal is
refused ("still being cleaned up") until the supervisor restarts, and a tmux process and a retry loop leak.

This is only possible, not demonstrated. The reviewer found no current product path that destroys a session under a
paused forwarder without first stopping that forwarder: delete stops forwarders before its kill, and restart's husk kill
only runs when the agent pane is already gone. The suggested change is the same as F5, applied in the output client's
shutdown: on a raw `can't find client` with the child still alive, close stdin and drain instead of retrying the
handshake.
