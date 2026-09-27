---
kind: fixed
---

Stopping or restarting a host's supervisor (`systemctl --user restart farhelm-supervisor`, an update, Ctrl-C on a foreground `farhelm supervisor run`, or quitting the desktop app) no longer risks ending every session on that host. The supervisor used to exit on the spot with every open terminal still streaming, which can make tmux's server abort and take all its sessions with it; it now closes each terminal stream properly first. A supervisor killed outright (out of memory, `kill -9`) still carries that risk.
