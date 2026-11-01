---
kind: none
---

Nothing user-facing yet: this adds the supervisor's side of starting terminal hosts (their private socket directory,
socket names, the tag that records what each host is for, and the start itself), and makes the test harnesses end any
hosts left in their state directories. No terminal is started in a host until a later change turns new terminals on;
that change carries the real entry.
