---
kind: fixed
---

A terminal tab that no longer exists on its host now says so and stops trying to reconnect. Before, it retried as if the connection had dropped, and a session listing many such tabs could keep the page busy for tens of seconds.
