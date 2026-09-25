# The supervisor's startup tmux -V check is unbounded

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

With a misconfigured tmux path, the supervisor can hang at startup instead of reporting the problem.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as
`F18 / COR-STARTUP-V-UNBOUNDED`, tagged **possible**. Anchors and title: `tmux.rs:2293`, `service/core.rs:5069` — The
supervisor's startup `tmux -V` check has no time or output limit

When the supervisor starts (core.rs:5069 calls `ensure_server`), `ensure_server_with_seam` first checks the configured
tmux program's version. It runs `<tmux> -V` with `self.command()…output().await` (tmux.rs:2293), with no deadline, no
kill on timeout, and all of stdout and stderr buffered in memory. This call does not go through `run_bytes`, so the
timeout proposed in F4 would not cover it. The same module's `probe_tmux`, which setup uses on the same kind of
operator-chosen program, is bounded to 5 s and 4 KiB per stream. Its docs give the reason: an unbounded probe of
something that may not be tmux can hang, or turn "a version check into an out-of-memory kill".

With a misconfigured `--tmux` or `FARHELM_TMUX` (a wrapper that blocks on a lock or a network mount, or a wrong binary
that ignores `-V` and runs interactively or prints without end), supervisor startup blocks forever or grows memory
without bound. It should instead fail with the clear "no tmux could be run" or below-floor message this function
otherwise produces. Under systemd the unit looks started while nothing answers. The premise is a program that misbehaves
on `-V`; real tmux answers at once. The suggested change is to reuse the bounded probe here (for example `probe_tmux`
via `spawn_blocking`, or the same deadline, capture limit and kill), and turn an overrun into a startup error that names
the program.
