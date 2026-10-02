---
kind: fixed
---

Adding a host that already runs a supervisor no longer goes wrong when the window that asked is reloaded or closed at
the wrong moment. The host used to be saved but never connected, and it stayed missing from the host list until the
helm restarted or another host was edited. The helm now finishes adding it either way.
