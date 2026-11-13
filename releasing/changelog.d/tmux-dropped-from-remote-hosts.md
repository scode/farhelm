---
kind: breaking
---

Remote Linux hosts no longer need tmux, and updating one removes the copy of tmux an earlier version of Farhelm
installed there. On a host that had that copy, `tmux` typed in a session now runs the host's own tmux, or is not found
if it has none.

A copy that an update before 0.18.0 put beside `farhelm` in a shared folder such as `~/.local/bin` is not removed, since
it may be the user's own, as 0.18.0's release notes said. On such a host `tmux` in a session still runs that old copy,
since the folder comes first on the `PATH` sessions start with.

Context for curation: Farhelm installed its own tmux on hosts whose tmux was older than 3.7c or missing, which covers
most remote Linux hosts, Ubuntu 24.04 and Debian 13 included. That copy lived in `~/.local/lib/farhelm/`, the directory
that also holds the host's `farhelm` and comes first on the `PATH` sessions start with, which is why `tmux` typed in a
session ran it; that was Farhelm's private tmux leaking into users' sessions, which this change ends. A tmux server a
user started from that copy keeps running from the deleted file until they end it. Adding a host again over an existing
install removes the copy too, and adding or updating a host no longer checks for tmux.

The setup and update plans show the removal as a step named "Remove the bundled tmux", which has nothing to do on a host
that never had the copy; if the removal fails, the step shows a warning and the update still completes. Uninstalling
Farhelm from a remote host no longer has a tmux step either. With this change nothing in Farhelm needs tmux on any
machine. Typed as breaking because a user who relied on `tmux` in sessions there may have to install their own; Farhelm
itself needs no action.
