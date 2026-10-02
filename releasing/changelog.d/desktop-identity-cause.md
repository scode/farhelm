---
kind: changed
---

When the desktop app refuses to start because its local supervisor reports a different identity than the app recorded,
the message now names the likely cause and what to do. The supervisor's database (`supervisor.db`) and the app's own
(`helm.db`) in the app's state directory no longer belong together, because one was restored from a backup or replaced
without the other, or `supervisor.db` was deleted. The message says to quit Farhelm, make sure no supervisor is still
running for that directory, and restore both files from the same backup.
