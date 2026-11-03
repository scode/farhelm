---
kind: none
---

Nothing user-facing yet: with new terminals turned on, restarting a session whose agent runs in tmux moves the agent to
a new terminal that keeps the tmux scrollback (not its last screen), while the session's open tmux tabs keep running
until they are closed. A move whose new terminal fails to start leaves the session in tmux as it was, ready for another
restart. Nothing turns new terminals on in a shipped supervisor yet, so every restart still stays in tmux; the change
that turns them on carries the real entry.
