---
kind: fixed
---

A session whose terminal host is gone shows its recovery card. If the host is temporarily unreachable, Farhelm can
still reconnect to it.

Delete now checks a terminal host's process even if its socket has disappeared. Delete and failed Restart also report
when a recorded process scope cannot be confirmed gone, so a partial cleanup doesn't look complete.
