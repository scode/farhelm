---
kind: fixed
---

A supervisor you started by hand with `farhelm supervisor run` now shuts down cleanly when its terminal closes or its
ssh connection drops, and so does the desktop app's own supervisor when the terminal the app was launched from closes.
Before, the hangup killed it on the spot, which can make tmux abort and take every session on that host with it. A
supervisor started under `nohup` still ignores the hangup and keeps running, as before.
