---
kind: none
---

Nothing user-facing yet: deleting a session now ends every terminal host the session has recorded (its agent's,
including one left behind by an earlier or interrupted launch) along with the programs in them, including a host that
is stuck and can only be ended by a signal. No session runs in a host until a later change turns new terminals on;
that change carries the real entry.
