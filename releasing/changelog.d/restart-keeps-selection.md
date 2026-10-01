---
kind: fixed
---

Restarting a session could leave the main area empty: the restart went through, but the session was no longer selected,
so its new terminal did not appear until you clicked the session again. It happened every time when restarting a
session that its host's reboot had interrupted, and could happen on other restarts. The session now stays selected
through the restart.
