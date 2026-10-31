---
kind: changed
---

Saving session and helm state now avoids a disk flush on every commit. An OS crash or power loss can lose the latest
saved changes; the databases remain consistent, and host identity creation and browser-token revocation still wait for
their changes to reach disk.

The databases now have accompanying `-wal` and `-shm` files. A backup that copies only `.db` files while Farhelm is
running can miss recent changes; restoration keeps the files included in that backup and removes current sidecars the
backup lacks.
