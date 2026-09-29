---
kind: fixed
---

Adding or updating a host from the hosts panel no longer fails when enabling linger fails for a reason Farhelm did not
recognize (for example `loginctl` missing or the system bus unreachable). Linger is optional: the step is now marked as
degraded, the supervisor then starts at login rather than at boot, and an update still restarts onto the new version.
