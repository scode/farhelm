# Non-dumpable daemons like ssh-agent escape the sweep without a user manager

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After deleting a session on such a host, an `ssh-agent` the agent started can keep running with the user's unlocked keys
while Farhelm says the session is gone.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F22 / SEC-NONDUMPABLE`, tagged **possible**. Anchors and title: `service/sweep.rs:320-323`, `service/sweep.rs:280-293`,
`procs.rs:1421-1423` — Stop and Delete cannot find non-dumpable daemons such as `ssh-agent` on hosts without a systemd
user manager

Once a process has left the session's process tree (daemonized and reparented to init), the kill sweep can find it only
by reading its environment for the `FARHELM_SESSION_ID` marker (sweep.rs:320-323, procs.rs:1421-1423). A program that
marks itself non-dumpable with `prctl(PR_SET_DUMPABLE, 0)` makes its environment unreadable even to its own user.
`read_environ` then returns nothing, and the sweep treats the process as unmarked. OpenSSH's `ssh-agent` does this by
default.

A reviewer verified it with a throwaway `FARHELM_SESSION_ID=... ssh-agent -a <tmp>/sock`. The agent detached, reading
`/proc/<pid>/environ` failed with EACCES, and `/proc/<pid>` stayed user-owned: the process is listed in the snapshot but
cannot be claimed.

An ordinary agent action like `eval "$(ssh-agent)"; ssh-add` therefore leaves a daemon holding decrypted keys. On Linux
hosts with a user manager, the launch's systemd scope catches it. On hosts without one, the sweep is the whole
mechanism, and Delete reports success while that daemon keeps serving keys. The gap is accepted only in a code comment
(sweep.rs:280-293). SPEC and SPEC_impl do not mention it, while SPEC says "Stop and delete reap everything the agent
started" and that teardown "covers ordinary agent descendants, including background servers".

A daemon holding decrypted keys outlives the session the user deleted, and Farhelm reports the session gone. That breaks
a promise about how long credentials stay live. It is not a cross-account breach.

The open premise is whether the maintainer accepts this gap, which so far only code comments do. Either record the gap
explicitly in SPEC_impl's process-tree section beside the macOS residual, or close it where there is no manager. One way
is to have the launch shim set `PR_SET_CHILD_SUBREAPER` before exec'ing the agent, so detached children reparent to the
agent and stay reachable by the tree walk. Another is an environment-independent membership signal scoped to the pane.

For the user, after deleting a session on such a host, an `ssh-agent` the agent started can keep running with the user's
unlocked keys while Farhelm says the session is gone.
