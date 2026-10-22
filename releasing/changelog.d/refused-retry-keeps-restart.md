---
kind: fixed
---

Restarting a session while Farhelm was still retrying the interrupted create that started it can no longer make the
restarted session disappear from the list or have its new agent stopped. Previously, if that retry was then refused, it
removed the session's record even though the session had already been restarted.

The exact timing needed to hit this was never confirmed in practice; the fix closes it either way.
