# probe_tmux can hang on a setsid'd descendant

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

With an unusual tmux wrapper on PATH, setup or the desktop app can freeze at launch.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as `F17 / COR-PROBE-HANG`,
tagged **possible**. Anchors and title: `tmux.rs:648-655`, `tmux.rs:708-709`, `tmux.rs:770-801`, `tmux.rs:812-839`,
`crates/farhelm-ui/src/desktop.rs:2181` — `probe_tmux` can hang forever despite its bounded-probe promise

`probe_tmux` runs `<candidate> -V` to check a tmux binary's version. `farhelm helm setup` and the desktop app's
pre-launch tmux check (desktop.rs:2181) both use it. Its docs (tmux.rs:648-655) promise a bounded result for any
candidate, because the candidate "is not necessarily tmux": a 5 s deadline, 4 KiB per output stream, and "a candidate
that spawns a descendant holding the captured pipes open cannot stall the probe either". The candidate runs in its own
process group, and each output stream is read on its own thread (`capture_bounded`, tmux.rs:812-839). On overrun, either
the deadline passing or a pipe still being held 250 ms after the candidate exits (tmux.rs:708-709), `ProbeGroup::retire`
(tmux.rs:770-801) sends SIGKILL to the whole process group and then joins both reader threads with no time limit. The
join assumes the kill closed every write end of the pipes ("with every writer dead, the pipes are closed").

That assumption fails for a descendant that has left the process group. A program that daemonizes calls `setsid()` or
`setpgid()`, for example `setsid somedaemon &` inside a wrapper script named `tmux`. Such a descendant survives the
group kill and keeps the inherited stdout or stderr open. If it stays silent, the reader thread blocks in `read_to_end`
and the join waits for as long as the daemon lives. A descendant that keeps writing would not cause a hang, because the
reader stops after 4 KiB + 1 bytes. Setup or desktop startup then freezes with no message, in exactly the
misbehaving-candidate case the function exists to report as `TmuxProbeError::Overran`. The premise is a tmux candidate
(in practice a wrapper) that leaves such a child behind; real `tmux -V` does not. The suggested fix is to stop joining
unconditionally after the group kill: wait on each reader's result channel with a short timeout, then drop the join
handle, which detaches the thread. Leaking a blocked thread in a short-lived CLI is acceptable where a hang is not. Also
correct the docstring and add a test whose fake candidate runs `setsid sleep 60 &` with stdout inherited.
