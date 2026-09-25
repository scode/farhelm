# Output-client shutdown targets the wrapper's pid behind a tmux wrapper

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

With such a wrapper, each terminal can be opened once and then stays "still being cleaned up" until the supervisor
restarts.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as
`F7 / COR-WRAPPER-OUTPUT-NAME`, tagged **possible**. Anchors and title: `tmux/stream.rs:300-307`, `tmux/stream.rs:1380`
— The output client's shutdown targets `client-<spawned pid>`, which never matches when tmux is behind a non-`exec`
wrapper

The shutdown handshake described in F5 and F6 has to name the client it is switching off. tmux names a pipe-backed
control client `client-<pid>`, where the pid is that of the tmux process that actually connected to the server. The
output stream builds the name from `child.id()` (stream.rs:300-307), the pid of whatever program Farhelm spawned, and
uses it at stream.rs:1380. Those are the same process only when the configured tmux program is tmux itself, or a wrapper
that `exec`s it. The operator can point Farhelm at any program with `--tmux` or `FARHELM_TMUX`, or with whatever `tmux`
comes first on `PATH`, and the version check passes through a wrapper script. With a wrapper like `#!/bin/sh` followed
by `/opt/tmux "$@"`, the wrapper forks tmux as a child. Reviewers verified this on 3.7c: the spawned pid was N, tmux
listed the client as `client-(N+1)`, and `refresh-client -t client-N` failed with `can't find client`.

From then on every orderly shutdown of an output client fails its first step while the wrapper process is alive.
`OutputReaper::run` retries forever. The `kill_on_drop` fallback would kill only the wrapper and leave the real tmux
client attached. Nothing fails at attach time, so the configuration looks healthy until the first detach, takeover, tab
close or restart of a terminal. After that, every attach to that terminal waits 15 s and fails with "the old terminal
attachment is still being cleaned up" until the supervisor restarts, and one tmux client leaks per detach. The open
premise is whether anyone runs a wrapper that does not `exec`; Homebrew symlinks and Nix `makeWrapper` wrappers both
`exec`.

The suggested fix is to stop inferring the name. Ask tmux for the client's real name during the attach exchange, while
output is still off: send `display-message -p '#{client_name}'` on the same control client and store the answer.
Alternatively, compare `#{client_pid}` with `child.id()` at open and refuse with a clear error ("the configured tmux
program must exec tmux"). The helper should be shared with F8.
