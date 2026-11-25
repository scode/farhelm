---
kind: fixed
---

Retrying a failed Restart keeps the launch record until cleanup is confirmed. This prevents the previous terminal
from reopening when the supervisor restarts while cleanup is still unconfirmed.

If an agent's terminal socket disappears during reconnection, Farhelm keeps the process identities it can still use
to clean up the session on Delete.
