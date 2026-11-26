---
kind: fixed
---

Stop and Restart now end an agent that kept running after its terminal host was killed outright, as Delete already did.
Before, an agent that ignored the hangup could survive Stop, and Restart could start the new agent beside it.

Farhelm still reads such a session as ended until one of those operations runs; it does not watch for survivors on its
own. The gap was a survivor outside any systemd scope whose session marker Farhelm could not read (for example on macOS,
or on a Linux launch made without a usable systemd user manager). When only a process the agent started ignored the
hangup, and the agent itself died, nothing changes: that process is still found only through its scope or marker.
