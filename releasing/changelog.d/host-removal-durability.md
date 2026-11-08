---
kind: fixed
---

Removing a host now durably removes its saved permissions too, so a power loss cannot bring the host back with its old
authority. On macOS, the existing limitation that these writes reach only the drive's cache still applies.
