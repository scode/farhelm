---
kind: fixed
---

Pi sessions on a host that had run Pi with Farhelm v0.12.0 or earlier can be resumed after a restart again, and the
agent is told about `farhelm agent` again. Since v0.13.0, Farhelm had quietly started Pi on those hosts without the
piece that reports which conversation it is in.
