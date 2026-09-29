---
kind: fixed
---

Removing a host while it was being set up or updated from the hosts panel could hang for as long as the update took,
indefinitely if its download or upload had stalled. Remove now answers at once: while an update is running it refuses
with a message saying the host is busy, and it can be removed once the update finishes.
