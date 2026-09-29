---
kind: fixed
---

On a host with systemd, Stop and Restart could report success, and Restart could start the new agent, even when
Farhelm could not confirm that the previous run's background processes were gone, so an old server could keep holding
its port next to the new agent. Both now report the failure instead, as Delete already did, and can be retried.
