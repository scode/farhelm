# A recycled pane pid can bind teardown's kill to an unrelated process tree

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

In an extremely rare timing window, deleting or archiving a session could kill processes belonging to something else
entirely on the same machine, rather than the session's own agent.

## Details

Source: pre-pr-review-swarm, area supervisor-teardown, 2026-09-17. Confidence: possible (recycling one exact pid in the
window needs extreme fork churn or a tiny pid_max; defense-in-depth, not an exploitable hole — an attacker cannot choose
the victim).

`teardown_session` probes the pane and keeps only a bare pid (teardown.rs:531-535 → :577:
`live_pane.filter(...).map(|pane| pane.pid)`). Between that probe and the kill it awaits tab rediscovery (:617-620) and
two systemd enumerations (:626-645) — several subprocess round-trips (~0.1–2 s). Only inside `reap_process_tree`
(sweep.rs:1278-1292) is the pid's starttime read and adopted UNCONDITIONALLY as the sweep seed; `enumerate_tree` admits
seeds without a marker check and expands the PPID closure downward from them. If the pane process exits and the kernel
recycles that pid number inside that window, reap binds the STRANGER's (pid, starttime) and signals
SIGTERM-through-SIGKILL against an unrelated same-uid process tree. Everything AFTER binding is airtight
(`signal_validated` re-reads starttime before each signal, sweep.rs:660-681; seed re-validation against the walk
snapshot; the scope-grace window already closed per sweep.rs:1071-1077) — but those all validate the BOUND identity, and
the bound identity is the stranger (genuinely alive at bind time, so every downstream check passes). Same shape in
`teardown_for_archive` (:198-202 probe → :239-240 bare pid → :252-307 enumeration plus upload joins → reap) and
`stop_live_agent` (sweep.rs:1726-1747, bare pid from the caller's even earlier liveness check with a SQLite intent write
in between). The impact is killing an unrelated process tree and the fix is cheap, and the codebase's own docs treat
this hazard class as real (starttime validation is everywhere downstream).

Suggested fix: capture `(pid, starttime)` via `procs::read_process` immediately after `pane_process` returns (before tab
rediscovery / scope enumeration / upload joins), thread `Option<(u32, u64)>` into `reap_process_tree`, and have reap
VALIDATE the carried identity against a fresh read — adopt on match, else `None` (marker scan alone). Same for
`stop_live_agent`: bind at the caller's liveness check instead of passing a bare pid.
