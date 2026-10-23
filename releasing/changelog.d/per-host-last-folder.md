---
kind: added
---

The session launcher now opens on the folder of your last agent launch on each host, so switching between, say, your
Mac and a Linux host no longer carries one machine's path over to the other. Changing the host switches to that host's
folder unless you typed or picked one.

A selected session on the same host still wins, and a host you have never launched on starts in its home folder.
Launches into a new managed checkout, and command launches, do not change the remembered folder.
