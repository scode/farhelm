---
kind: none
---

Nothing a user can notice: the helm now drops a pause or resume that a page sends on a new terminal's socket instead of
forwarding it to the supervisor, since a new terminal's flow control is the helm's own. Farhelm's own page never sends
one there, so only a page that does (none ships) would have held a new terminal paused until it detached as stalled.
