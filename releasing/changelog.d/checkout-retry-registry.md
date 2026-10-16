---
kind: fixed
---

After recovering with a fresh helm database, an agent can no longer reuse another host's earlier approval to create a
session in a managed checkout. A keyed retry already in flight across this upgrade needs approval again and, if approved,
creates a new session or checkout instead of returning the earlier one.
