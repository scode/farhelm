# BUGS

Known bugs we are aware of but have no intention of fixing, documented because they can still bite users. This is
deliberately not TODO.md: entries there describe work that is still wanted, while entries here describe behavior we have
decided to live with — usually because the defect lives below farhelm (tmux, the kernel, a library) or because the only
fix would cost more than the bug does. An entry moves out of this file only if that calculus changes.

## Abrupt supervisor death can crash the private tmux server, killing every session

This applied while the supervisor ran its terminals in a private tmux server. It runs none any more (old terminals are
ended at its start), so a supervisor death can no longer take sessions with it this way. The entry stays because the
installation's tmux version floor cites it (`crates/farhelm-supervisor/src/tmux.rs`).

If the supervisor died without running any cleanup — SIGKILL, OOM kill, a segfault — while its tmux control clients
still had queued output, the private tmux server itself sometimes aborted. When that happened every session on that
server died with it: agent processes, tabs, scrollback, all of it. Farhelm's durable state survived (the sessions were
rediscovered as exited on the next supervisor start), but the running work in those sessions was gone.

Here's the TLDR of the mechanics. The kernel closes a dead process's file descriptors, so the server saw EOF and EPIPE
on the dead supervisor's control-client pipes, some of which still carried queued pane output. tmux accounts that queued
output in blocks, and the teardown that follows can race the accounting into a state where a buffer holds fewer bytes
than its own bookkeeping claims. tmux's codebase uses `fatal()` the way other projects use assertions: on an internal
invariant violation it aborts the whole server rather than continue with state it no longer trusts. Reaching the
corrupted state at all is tmux's bug, not ours; a server should treat "a client died mid-block" as routine.

What we observed and how sure we were of it: 5 aborts in roughly 5,000 supervisor-SIGKILL iterations on a saturated
4-core box running distro tmux 3.6 (2026-08-18), each leaving "no server running" behind with no farhelm-side error. The
identification with tmux's `fatal: not enough data` abort — the same class the pinned-3.7b regression suite existed for,
where it was triggered by unsafe live teardown of an output-bearing control client — was inferred from the matching
trigger shape, not captured directly: the dying server's stderr was not being recorded. It was never established whether
tmux 3.7b's teardown fixes cover this post-mortem path, nor which distro versions are affected beyond 3.6. The archived
M6.5 flake ledger (under lore/) had independently seen the private tmux server die under load before the trigger was
understood.

Why it was not fixed: there was nothing to fix on our side of the line. Farhelm's own teardown paths disabled output
through an acknowledged boundary before closing any control client, precisely to avoid this abort, but a SIGKILLed
supervisor runs nothing. A planned stop (SIGTERM, SIGINT, SIGHUP, or the desktop app's closing tether) ran that orderly
shutdown, so only a death that runs no code was left, and the only code that could have made post-mortem teardown safe
was tmux's.

## A host install or update can hang until the helm restarts if a local ssh helper keeps its output open

If the helm stops a stalled or failed transfer of Farhelm's files to a host while a local helper started by ssh still
holds ssh's output open, the install or update never finishes. The host stays mid-install or mid-update (its row keeps
showing the run in progress) until the helm restarts: renaming it or changing its destination waits, removing it is
refused as busy, and one of the helm's four install slots stays taken meanwhile. Stopping the transfer kills ssh's whole
process group, so only a helper that left it can do this, such as an ssh `ProxyCommand` that daemonizes.

Here's the TLDR of the mechanics. Each provisioning step runs a local process (`ssh` for a remote host) and reads its
output to the end. After that process exits, and after any step other than the transfer kills it, the helm stops waiting
for the output to end after ten more seconds and fails the step, so a process that inherited an output pipe and lingers
costs a delay or an error. The transfer's reads after a kill have no deadline, and neither does the task feeding the
files into ssh. Remote leftovers cannot cause this: they keep `ssh` itself from exiting, and that wait has a deadline.

How sure we are: this comes from reading the code; nobody has seen it happen. Farhelm's own provisioning scripts start
no background processes, and a check with OpenSSH 9.6p1 showed that ssh's connection sharing (a `ControlPersist` master
that stays alive after the command) does not hold the output open: the read finishes within about half a second. Older
OpenSSH releases were not checked.

Why we are not fixing it: there is no realistic trigger on current OpenSSH, and what is left needs both a stopped
transfer and a helper outside ssh's process group.
