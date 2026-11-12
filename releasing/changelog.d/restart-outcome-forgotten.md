---
kind: fixed
---

Opening a session after its host rebooted now shows that the session was interrupted, with its Restart button, even when
an earlier Restart of that session had finished while you were looking at another one. Before, the app could instead try
to show the session's terminal, which the reboot had already destroyed, because it still remembered that earlier
Restart.

This needed two reboots of the same host: one before the Restart, and another before you next opened the session. A
second reboot that comes before the session list has shown the restarted session running still has the old problem;
that gap was accepted when the fix was planned.
