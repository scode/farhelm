---
kind: fixed
---

Remote Mac setup checks that the host has the clock command needed to stop its background service. A failed Tailscale
connection keeps its own error, and a terminated SSH or Podman connection fails the operation even when the remote
command reported success.
