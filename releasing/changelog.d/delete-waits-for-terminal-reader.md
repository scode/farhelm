---
kind: fixed
---

Deleting a session whose terminal was open and had produced a lot of output no longer occasionally leaves a stuck
background tmux client behind on that host, retrying every five seconds until the host's Farhelm was restarted. Delete
now lets the terminal's output reader shut down cleanly before it removes the session.
