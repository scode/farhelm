---
kind: fixed
---

A terminal producing many clipboard updates can no longer leave an unbounded backlog of old writes in the Farhelm window. At most one write is applied at a time, and a newer value replaces an older pending one.
