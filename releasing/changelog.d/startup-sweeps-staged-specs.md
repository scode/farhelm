---
kind: fixed
---

When the supervisor starts, it now removes leftover partial copies of session launch files that a crashed supervisor left behind, even for sessions that still exist. Those copies held a session's command line and access token until the session was deleted.
