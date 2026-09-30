---
kind: fixed
---

An `--idempotency-key` passed to `farhelm agent create` or `farhelm agent clone` now belongs to the session that ran the
command. Before, another session reusing the same key and request, such as a child re-running its parent's command, got
the earlier result back, which could be the asking session itself reported as the new one. Keys used before this
release are not recognized afterwards, so a retry spanning the upgrade creates a new session.
