# Deleting a busy session can leave the sink retrying forever

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After deleting a busy session, a background tmux client and retry loop linger, and a same-named session recreated by
restart may become unattachable.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as
`F5 / COR-SINK-STUCK-DELETE`, tagged **definite**. Anchors and title: `tmux/sink.rs:256`, `tmux/sink.rs:284`,
`service/terminals.rs:716`, `service/terminals.rs:1175`, `service/teardown.rs:355`, `service/teardown.rs:440-444` —
Deleting a busy session can leave its sink client in an endless shutdown-retry loop

Some background first. A tmux control-mode client (`tmux -C attach`) is a tmux client that talks a line protocol over
pipes instead of drawing a terminal. Farhelm keeps one such client per viewed session, called the sink, whose only job
is to keep reading and discarding pane output so tmux never throttles a pane. Closing a control client while tmux still
has pane output queued for it can crash the whole tmux server; the module docs cite tmux 3.7b's
`fatal: not enough data`. So every orderly shutdown first runs a separate
`tmux refresh-client -t client-<pid> -f no-output` command, and only after that command succeeds does it close the
client's stdin and drain its stdout to end-of-file. If the `refresh-client` fails while the client process is still
alive, `SessionSink::shutdown` (sink.rs:256, error return at :284) returns an error without draining.
`shutdown_session_sink_until_safe` (terminals.rs:716, driven from the shutdown arm at :1175) then retries the same
sequence forever, backing off to every 5 s.

Delete triggers this in a racy order. In `teardown.rs` the last sink reference is dropped at :355. Its `Drop` only
spawns the sink shutdown in the background, and delete goes straight on to `kill_session` at :440-444 without waiting.
If tmux destroys the session first, `refresh-client -t client-<pid>` answers `can't find client`, because tmux no longer
lists a client that has lost its session. The client process cannot exit either. tmux holds a control client's exit
until its queued output has been written to the pipe, and nobody reads the pipe any more, because the sink stopped
draining when it began shutting down. The shutdown code never drains without a successful handshake, so the loop can
never end. The reviewer reproduced this on 3.7c with a `yes` pane. In 2 of 10 trials that ran the refresh and
`kill-session` concurrently, as delete does, the refresh was refused and the client stayed alive. Draining the pipe by
hand (about 52 KB) let the client exit cleanly. The trigger only needs the session's panes to write more than about 64
KiB between the sink's last read and the kill, which a build log or `cat` does within milliseconds.

For an ordinary delete the harm is a leaked tmux client process and a `refresh-client` spawned every 5 s until the
supervisor restarts. The sink registry entry for that tmux name also stays in the `Reaping` state. That becomes serious
on the restart path that kills and recreates a tmux session under the same name (the "husk" path, core.rs:~10415). If
the race hits there, every later attach to the recreated session waits 15 s in `ensure_session_sink` and fails. Two
fixes are suggested, and either or both would work. When the handshake fails with tmux's raw `can't find client` and the
process is still alive, go straight to closing stdin and draining (`shutdown_output_control_client`); at that point tmux
has already begun the client's exit and is only waiting for the drain. Alternatively, have delete wait for the sink's
reap to finish before calling `kill_session`, so the handshake always happens while the session still exists.
