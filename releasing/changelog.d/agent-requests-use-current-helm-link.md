---
kind: fixed
---

`farhelm agent` commands no longer sometimes time out after 30 seconds, or report an unknown outcome, after the helm reconnects to a host. When the helm's old connection to that host was left half-open, some of a session's terminals could still be tied to it, and an agent command from that session could be sent down the dead connection instead of the working one.
