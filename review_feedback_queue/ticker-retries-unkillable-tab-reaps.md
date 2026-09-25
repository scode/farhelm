# The ticker retries unkillable dead-tab reaps every tick

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

One tab with an unkillable leftover can make that session's operations repeatedly stall or refuse and delay background
updates for all sessions.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F20 / COR-TICKER-RETRY`, tagged **possible**. Anchors and title: `service/ticker.rs:1444-1511`,
`service/core.rs:11722-11735` — The ticker retries unkillable dead-tab processes with full graces every tick, starving
the ticker and holding the session's lifecycle claim

The supervisor's ticker is its periodic background loop, running about every two seconds. It samples activity, captures
conversation state, and, through `reap_dead_tabs` (ticker.rs:1444-1511), automatically closes tabs whose shells have
exited. It calls `close_tab` serially on its own task for up to four dead tabs per tick.

Suppose a tab holds a process that cannot die, for example one stuck in uninterruptible sleep on a hung NFS or FUSE
mount, which ignores even SIGKILL until the kernel call returns. Each attempt then costs about 14 seconds. The scope
kill takes up to a 5-second SIGTERM grace, SIGKILL and a 2-second confirmation. The process sweep then takes another
5-second grace, the quiesce passes, SIGKILL and a 2-second confirmation. The close fails before `kill_window`
(core.rs:11722-11735), so the dead window remains and the next tick tries again, indefinitely. Every attempt holds the
session's lifecycle claim, the per-session lock that serializes stop, restart, delete and tab operations.

The ticker's own docs say neither a mass exit nor a slow teardown can wedge sampling, capture or shutdown. With one
stuck tab, though, the ticker spends most of its time retrying, which delays activity sampling and conversation capture
for every session. Meanwhile stop, delete and tab-attach on the affected session keep running into the held claim and
are told the session is "being stopped or deleted; retry".

The premise is a tab process that survives SIGKILL after its shell exited. Back off per tab after a failed reap, with an
exponential delay or by skipping it for N ticks. Optionally, run tab reaps off the ticker task under a separate bound.

For the user, one tab with an unkillable leftover can make that session's operations repeatedly stall or refuse, and
delays background updates for all sessions.
