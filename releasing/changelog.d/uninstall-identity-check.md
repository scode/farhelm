---
kind: fixed
---

Uninstalling a remote host now refuses when the machine answering at its address reports an identity while Farhelm has
none recorded for that host, the same way it already refused a machine reporting a different identity or none at all.
Uninstall removes files, so it only goes ahead when it can tell it reached the machine whose sessions it just checked.
