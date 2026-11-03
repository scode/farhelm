---
kind: none
---

Nothing user-facing yet: terminal tabs can now open in terminal hosts of their own, beside an agent in either kind of
terminal, and are listed, attached, closed, reaped when their shell exits, ended by Delete and kept across supervisor
restarts. Only the supervisor's internal switch opens them, which no shipped supervisor turns on; the change that turns
new terminals on carries the real entry.
