---
kind: fixed
---

A host whose supervisor kept saying its sessions had changed could make the helm re-read that host, and every open
browser and desktop window re-read the session list, as fast as the network allowed. The helm now answers those notices
at most five times a second per host, which is as often as an ordinary supervisor ever sends them.
