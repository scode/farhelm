---
kind: fixed
---

Farhelm now gets the result of each host setup command from the command itself, so adding or updating a host can work
with SSH servers that report success for failed commands, including affected Tailscale SSH servers on macOS. Farhelm
still uses the system SSH client.

Context for curation: this has not been tried against a real Tailscale SSH server on a Mac; the tests stand in an SSH
connection that reports success for failed commands, and the real Tensorlake check was skipped. Only host setup commands
changed. A live connection that fails to start over such a server still reports a generic error rather than the real
cause, and Tailscale SSH's check mode and the sessions it leaves behind are untouched.
