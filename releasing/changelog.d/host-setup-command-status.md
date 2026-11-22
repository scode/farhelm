---
kind: fixed
---

Farhelm now gets the result of each host setup command from the command itself, so adding or updating a host can work
with SSH servers that report success for failed commands, including affected Tailscale SSH servers on macOS. Farhelm
still uses the system SSH client.
