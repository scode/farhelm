---
kind: fixed
---

`farhelm spawn` run by an agent session no longer reports that session itself as the newly created child. A child started with `--inherit-agent` that re-ran its parent's spawn command with the same `--idempotency-key` used to get its own id back, so stopping or restarting "its child" hit itself. That spawn is now refused with a conflict asking for a fresh key, and nothing is created.
