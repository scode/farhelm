# A hangup kills the supervisor without its orderly shutdown

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

Closing the terminal window that launched the desktop app, or losing the terminal or ssh connection of a hand-started
supervisor, kills the supervisor abruptly instead of shutting it down cleanly, which per BUGS.md can crash the private
tmux server and take every running session with it.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F34 / COR-SIGHUP`, tagged **definite**. Anchor and title:
`crates/farhelm-supervisor/src/service/core.rs:14390` — SIGHUP kills the supervisor immediately, skipping its orderly
tmux output shutdown.

When the supervisor is asked to stop, it is supposed to go through an orderly shutdown before exiting: it closes its
tmux "output clients" (the background tmux connections that stream each session's terminal output to the supervisor) one
by one, through an acknowledged handoff. That ordering exists because of a known tmux bug recorded in BUGS.md ("Abrupt
supervisor death can crash the private tmux server, killing every session"): if the supervisor dies while those clients
still have queued output, the private tmux server sometimes aborts, and every session on the host dies with it. BUGS.md
says the only deaths that skip the orderly path are ones that run no code at all (SIGKILL, an OOM kill, a crash), and
that a planned stop — SIGTERM, SIGINT, or the desktop app closing its stdin "tether" to a supervisor it manages — always
runs the orderly shutdown.

That statement leaves out SIGHUP. The supervisor's main loop (`run` in `crates/farhelm-supervisor/src/service/core.rs`,
around lines 14390–14398) installs listeners only for SIGTERM, SIGINT and the tether. Nothing anywhere in the supervisor
or the `farhelm` binary installs a SIGHUP handler or ignores the signal, so SIGHUP keeps its default action, which is to
terminate the process immediately — exactly the "runs no code" kind of death the orderly path exists to avoid. The tmux
output clients are spawned by the supervisor without a separate process group, so they share its group and can receive
the same hangup.

A hangup is not exotic. It is what a process gets when the terminal it was started from goes away: closing the terminal
window from which someone launched the desktop app (the desktop spawns its managed supervisor as a plain child in the
app's own process group — `crates/farhelm-ui/src/desktop.rs`, around lines 380–398, sets no separate group), or losing
the terminal or ssh connection of a hand-started `farhelm supervisor run`. In those cases the supervisor dies abruptly,
and per BUGS.md the private tmux server may abort and take every running agent, tab and scrollback with it. The absence
of handling is certain; whether the tmux crash follows in a given instance is the probabilistic part.

The fix is to add a `SignalKind::hangup()` listener next to the SIGTERM and SIGINT ones and route it into the same
orderly shutdown. Optionally, the desktop app could also start its managed supervisor in its own process group
(`process_group(0)` or `setsid`), so a terminal hangup aimed at the app reaches the supervisor only through the tether,
which already triggers the orderly path. BUGS.md's description of which deaths skip the orderly path should be updated
to match whatever is chosen.
