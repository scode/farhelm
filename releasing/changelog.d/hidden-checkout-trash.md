---
kind: changed
---

Managed checkout trash now lives in hidden `.farhelm-trash`, and existing trash moves there automatically. Sessions
started inside the old trash keep their old working-directory path, so restarting them can fail after the move; they
still protect those checkouts from permanent deletion.
