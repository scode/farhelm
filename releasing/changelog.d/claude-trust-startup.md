---
kind: fixed
---

Leftover cleanup of Claude's managed-checkout trust entries no longer waits for Claude's settings lock while Farhelm
starts. If the lock is busy, cleanup is left for a later start.
