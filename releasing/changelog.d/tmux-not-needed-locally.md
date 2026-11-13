---
kind: breaking
---

The Mac app and a Linux machine running the helm no longer need tmux, and `farhelm helm setup --tmux` is removed: a
script that passes it has to drop it, or setup refuses to run.

The app starts without tmux instead of refusing, `farhelm helm setup` no longer looks for one, and the installer no
longer checks your Mac for tmux or tells you to install it with Homebrew.

The supervisor service that `farhelm helm setup` and adding a remote Linux host write no longer names a tmux or puts its
directory on the `PATH` sessions start with. A service written by an earlier release keeps its `FARHELM_TMUX` line until
the next `farhelm helm setup` or Update rewrites it; the line is ignored. Where a tmux you installed yourself lived in a
directory that only that `PATH` entry made reachable (Linuxbrew's, say), the programs in that directory, such as
`tmux`, are found from a session on that machine only if your login shell's own `PATH` includes it.

Setting up a remote Linux host still checks its tmux and installs Farhelm's own copy when it has no new enough one.

Context for curation: the supervisor itself finds `systemd-run`, `systemctl` and `git` through that same `PATH`, which
is now only its own directory then `/usr/local/bin:/usr/bin:/bin`. On a machine that keeps those programs elsewhere,
such as NixOS, and whose tmux lived in that same directory, the old service found them only through the tmux entry;
after setup or Update rewrites it, sessions there start without their systemd scope and repository discovery fails.
Hosts that got Farhelm's own tmux never had that entry, so this was never designed coverage, but a curator may want to
mention it.
