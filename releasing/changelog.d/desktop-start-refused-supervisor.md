---
kind: fixed
---

The desktop app no longer waits 30 seconds and then fails with "managed local supervisor did not connect" when the
supervisor already running in its state directory cannot be used. If it runs an incompatible Farhelm version, typically
one you started yourself before upgrading, the app now stops at once, names both versions, and tells you to stop that
supervisor and start Farhelm again. If it reports a different identity from the one recorded for this machine, or none,
the app also stops at once and says so. The message no longer calls a supervisor "managed" when the app did not start it.
