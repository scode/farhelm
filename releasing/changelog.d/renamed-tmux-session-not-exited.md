---
kind: fixed
---

If a session's tmux session was renamed or its pane moved by hand on Farhelm's private tmux server, the next session
list, or the next supervisor start, recorded the still-running agent as exited. It now shows as unknown instead, the
same way stop and restart already refused to treat it as exited.
