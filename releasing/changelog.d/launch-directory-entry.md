---
kind: fixed
---

Starting or restarting an agent, replacing a session, or opening a terminal tab now fails with an error naming the session folder when it cannot be entered, instead of silently running in the home folder. Agent launches also return to the session folder if shell startup files change directories; terminal tabs keep their shell's normal startup behavior.
