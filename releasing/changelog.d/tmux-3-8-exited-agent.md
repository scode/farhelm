---
kind: fixed
---

Restart, Replace, Stop and Delete now work on a session whose agent has already exited when the host runs tmux 3.8.
Before, each attempt failed with "parsing pane_pid … cannot parse integer from empty string".

tmux 3.8 (released 2026-09-09) stopped reporting a process id for a terminal whose program has exited, and Farhelm
expected one. Only hosts whose Farhelm tmux server runs 3.8, such as one picked up from Homebrew or Linuxbrew, were
affected; Farhelm's own tmux build is 3.7c. Sessions whose agent was still running were not affected. One remaining
difference on tmux 3.8: a status report from an agent that arrives after its terminal already shows "Pane is dead" is
ignored.

For the curator: only Restart was reported from the field. Replace, Stop and Delete were found by reading the code,
which asks tmux the same question first in each.
