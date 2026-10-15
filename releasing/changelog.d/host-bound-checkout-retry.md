---
kind: fixed
---

An agent on one host can no longer finish an `agent create` into a managed checkout that you approved for another host's session by repeating that session's ID and `--idempotency-key`; it is asked about like any other request. A retry that was pending across this upgrade is asked about again.
