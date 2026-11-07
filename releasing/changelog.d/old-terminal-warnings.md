---
kind: changed
---

An upcoming Farhelm release will forcibly terminate anything still running in an old terminal (one still run by tmux).
The note at the top of such a session now asks you to restart, replace, stop or delete it as soon as possible, and to
close and reopen any of its shell tabs that are still old terminals, since restarting a session restarts only its
agent, not its tabs. A session whose agent has stopped and that has no old tabs no longer shows the note.

The release that removes old terminals is not named and has no date yet; the entry must not name one.
