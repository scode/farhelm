# A SIGSTOPped process missing from the final enumeration stays frozen

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After a stop or delete (sometimes one that reports an error, sometimes one that reports success), one of the session's
processes can be left frozen, e.g. a dev server still holding its port.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F10 / COR-FROZEN`, tagged **possible**. Anchors and title: `service/sweep.rs:1114-1147`, `service/sweep.rs:702-733`,
`service/sweep.rs:608-613` — A process frozen with SIGSTOP but missing from the final enumeration gets neither SIGKILL
nor SIGCONT and stays frozen

The kill sweep, `kill_process_tree`, escalates in stages. It sends SIGTERM and waits a grace period. It then sends
SIGSTOP to everything it finds ("quiesce", so nothing can fork during teardown) and re-enumerates up to five times,
stopping anything new. Finally it SIGKILLs and confirms the processes are gone (sweep.rs:1114-1147). A
`StoppedProcessGuard` (sweep.rs:702-733) records every identity it froze and sends SIGCONT to all of them, but only if
the sweep is cancelled before the SIGKILL.

The SIGKILL goes to `found.identities`, which is only the last enumeration's result, and the guard is disarmed right
after. An enumeration re-admits a previously found process only if the new process-table snapshot still lists it with
the same start time (sweep.rs:608-613). A process frozen in an earlier pass can drop out of the last one in two ways:

- A soft read error on that pid's `/proc` row omits it from the snapshot.
- On Linux, a descendant that exec'd a setuid binary such as `sudo` right before being stopped now has a different
  effective uid. The snapshot keeps only processes owned by the supervisor's effective uid, so it is filtered out with
  no error.

Such a process receives neither SIGKILL nor SIGCONT and stays frozen indefinitely. In the soft-error case the sweep at
least reports an error. In the setuid case it can return success.

The guard exists so the sweep never leaves frozen processes behind, but it is disarmed over a set smaller than the set
it froze. The frozen process keeps holding its locks, ports and files until someone intervenes by hand.

The premise is that a stopped process drops out of the final enumeration while still alive. The fix is to send the final
SIGKILL, and run the confirmation, over the union of `found.identities` and every identity the guard recorded.
`signal_validated` still re-checks start times, so signalling a stale identity is safe. Disarm the guard only after
that, or SIGCONT anything it recorded that was not killed.

For the user, after a stop or delete, sometimes one that reports an error and sometimes one that reports success, a
process of the session can be left frozen, for example a dev server still holding its port.
