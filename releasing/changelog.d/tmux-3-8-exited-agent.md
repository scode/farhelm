---
kind: fixed
---

With tmux 3.8, Restart, Replace, Stop and Delete now work on a session whose agent has already exited, and opening or
closing a terminal tab whose shell has exited no longer fails. Before, these failed with "parsing pane_pid … cannot
parse integer from empty string". On a Mac this affected anyone whose Homebrew tmux was updated to 3.8; on Linux, only
hosts using a tmux 3.8 from outside Farhelm, such as Linuxbrew's, since Farhelm's own build there is 3.7c.

One gap remains with tmux 3.8: if an agent switches to a different conversation and exits before Farhelm has noted the
switch, a later Restart can resume the conversation it had before.

For the curator: tmux 3.8 (released 2026-09-09 per its GitHub release) stopped reporting a process id for a terminal
whose program has exited. Only Restart was reported from the field; Replace, Stop, Delete and the tab cases were found
by reading the code, which asks tmux the same question first in each. The remaining gap is that a pending conversation
report can no longer be tied to the session once its agent has exited.
