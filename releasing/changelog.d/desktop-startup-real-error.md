---
kind: fixed
---

When the desktop app fails to start after its built-in helm is already running (for example because the local supervisor never connects), it now reports that actual error instead of, or in addition to, "embedded helm stopped unexpectedly". On macOS this also avoids a second, misleading alert.
