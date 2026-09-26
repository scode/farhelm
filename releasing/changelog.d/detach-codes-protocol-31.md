---
kind: changed
---

The helm and supervisors now speak protocol version 31. After updating, update the supervisor on each of your hosts:
until then those hosts refuse to connect and show that they need an update. Nothing else changes for you: a terminal
that another client takes over, one that stops reading its output, or a tab that closes still behaves as before.
