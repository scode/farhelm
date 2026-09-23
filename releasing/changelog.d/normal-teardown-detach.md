---
kind: fixed
---
Closing a terminal whose supervisor connection is wedged now gives local teardown a five-second grace period to enqueue
its detach, so the browser close does not retain the handler for the full connection stall window. The detach
notification remains independently owned and can still reach the supervisor if the writer recovers.
