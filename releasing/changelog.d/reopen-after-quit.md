---
kind: fixed
---

Reopening the Farhelm app right after quitting it no longer fails to start. The Farhelm you just quit can take a few
seconds to finish closing its sessions' terminal connections, and the reopened app used to give up instead of waiting
for that. It now waits up to about 20 seconds.
