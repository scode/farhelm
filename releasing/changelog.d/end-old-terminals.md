---
kind: breaking
---

Updating to this release ends the old terminals, the ones that still ran in tmux, as earlier releases warned. A session
whose agent was still running in an old terminal now shows as interrupted, with a notification saying so: Restart starts
the agent again in a new, empty terminal and resumes its conversation, and where Restart is unavailable, Replace
starts it anew. Old shell tabs are closed.

A session whose agent had already exited keeps its status, but its terminal no longer shows the agent's last screen,
which lived in the old terminal. Programs that old agents and tabs left running in the background are stopped where
Farhelm can tell which session they belong to; the rest are stopped when their session is deleted. The earlier warning
notifications now show as resolved, and the installer's update message no longer says that sessions keep running.

Creating a session that an older release was still starting in an old terminal when it was interrupted (by a crash, say)
is no longer retried: the retry is refused, and the session has to be created again.

If Farhelm cannot end the old tmux (it logs why), it starts anyway and treats those sessions as ended, so a Restart could
start a second agent beside one that is still running. Remote hosts end their old terminals when they are updated.

`farhelm supervisor run` no longer accepts `--tmux`, since the supervisor no longer runs tmux at all. A service unit or
script you wrote yourself that passes it has to drop it, or the supervisor will not start; the units that
`farhelm helm setup`, the desktop app and adding a remote host write never pass it.
