---
kind: fixed
---

When a host's earlier install had failed and a retry of it had just been confirmed, adding the same host again from
another browser window could, in a race lasting a few milliseconds, start a second install or update at the same time
instead of being refused, with the two overwriting each other's progress in the host list. The second one is now
refused while the first runs.
