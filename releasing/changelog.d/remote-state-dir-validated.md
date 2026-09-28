---
kind: fixed
---

Registering a remote host, whether by adding it in the UI, through `--ensure-hosts`, or after a probe, now refuses an empty remote state directory or one containing a NUL byte. Such a host used to register and then never connect, with an error about something else, and the only way out was to remove it and add it again, losing its cached sessions.
