---
kind: fixed
---

Removing a session container now asks Podman to clean up its unused anonymous volumes, which could previously keep
using disk space on the container host.
