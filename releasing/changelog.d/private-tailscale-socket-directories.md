---
kind: fixed
---

Farhelm uses a private SSH socket directory for each Tailscale destination, so a folder created by another account won't prevent a connection. Changing a Tailscale host's destination or removing the host also retires its old shared SSH connections.
