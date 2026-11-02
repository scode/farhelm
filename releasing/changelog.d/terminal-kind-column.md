---
kind: none
---

Nothing user-facing yet: each session now records in the supervisor's database whether its terminal runs in tmux or in
a terminal host, and every existing session is recorded as a tmux session. No terminal runs in a host until a later
change turns new terminals on; that change carries the real entry.

For the curator: this raises the supervisor database schema (31 to 32), which the release's downgrade check reports.
Per the editorial guidance no entry says the release cannot be downgraded from.
