---
kind: fixed
---

"Replace with" now says what it is about to stop. Its launcher used to delete the source session whatever it was
running, with nothing on screen saying so. While the source has a running agent or open terminal tabs, the launcher now
shows Replace's warning under its replace button, and keeps it current while it stays open. Launching authorizes only
what that warning said: if the source was restarted after the launcher last showed it (by the command line, an agent or
another window), the new session is still created, the source is kept, and the launcher says both sessions exist.
