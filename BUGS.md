# BUGS

Known bugs we are aware of but have no intention of fixing, documented because they can still bite users. This is
deliberately not TODO.md: entries there describe work that is still wanted, while entries here describe behavior we have
decided to live with — usually because the defect lives below farhelm (tmux, the kernel, a library) or because the only
fix would cost more than the bug does. An entry moves out of this file only if that calculus changes.

## Abrupt supervisor death can crash the private tmux server, killing every session

If the supervisor dies without running any cleanup — SIGKILL, OOM kill, a segfault — while its tmux control clients
still have queued output, the private tmux server itself sometimes aborts. When that happens every session on that
server dies with it: agent processes, tabs, scrollback, all of it. Farhelm's durable state survives (the sessions are
rediscovered as exited on the next supervisor start), but the running work in those sessions is gone.

Here's the TLDR of the mechanics. The kernel closes a dead process's file descriptors, so the server sees EOF and EPIPE
on the dead supervisor's control-client pipes, some of which still carry queued pane output. tmux accounts that queued
output in blocks, and the teardown that follows can race the accounting into a state where a buffer holds fewer bytes
than its own bookkeeping claims. tmux's codebase uses `fatal()` the way other projects use assertions: on an internal
invariant violation it aborts the whole server rather than continue with state it no longer trusts. That fail-fast
stance is defensible — limping on with a corrupted output queue could interleave garbage into every session — but
reaching the corrupted state at all is the actual bug, and it is tmux's, not ours. A server should treat "a client died
mid-block" as routine.

What we have observed and how sure we are of it: 5 aborts in roughly 5,000 supervisor-SIGKILL iterations on a saturated
4-core box running distro tmux 3.6 (2026-08-18), each leaving "no server running" behind with no farhelm-side error. The
identification with tmux's `fatal: not enough data` abort — the same class our pinned-3.7b regression suite exists for,
where it is triggered by unsafe live teardown of an output-bearing control client — is inferred from the matching
trigger shape, not captured directly: the dying server's stderr was not being recorded. We have not established whether
tmux 3.7b's teardown fixes cover this post-mortem path, nor which distro versions are affected beyond 3.6. This is also
not the first sighting: the archived M6.5 flake ledger (under lore/) independently confirmed the private tmux server
dying under load — "%exit server exited unexpectedly", both probes reporting no server — before the trigger was
understood.

Why we are not fixing it: there is nothing to fix on our side of the line. Farhelm's own teardown paths already disable
output through an acknowledged boundary before closing any control client, precisely to avoid this abort — but that
discipline is code the supervisor runs, and a SIGKILLed supervisor runs nothing. No handler survives SIGKILL. The only
code that could make post-mortem teardown safe is tmux's. What farhelm does do is clean up the non-crash aftermath: when
the server survives an abrupt supervisor death, the next supervisor start reaps any control clients the dead one left
wedged (see `reap_stale_control_clients` in the supervisor's tmux driver for why they can wedge forever).

If you hit this, the signature is: all sessions gone at once, `no server running on <state-dir>/tmux.sock`, and a
supervisor that restarts cleanly into an empty server with your sessions listed as exited. Reporting it upstream to tmux
would need the crash site pinned first — running the server with `-vv` logging under the same kill-under-load recipe
would capture the fatal message and call site.
