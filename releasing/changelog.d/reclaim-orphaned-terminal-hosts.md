---
kind: none
---

Nothing user-facing yet: when the supervisor starts, it now shuts down any terminal host left running for a session or
terminal tab it knows nothing of, and removes the files that hosts which already ended left behind. A host whose session
still has a launch record is left running even when the session database no longer lists it (as after restoring an
older copy of the database), and so are hosts it cannot read and every host while its session database is empty. No
session runs in a host until a later change turns new terminals on; that change carries the real entry.
