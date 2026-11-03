---
kind: none
---

Nothing user-facing yet: the supervisor can now start a new session's agent in a terminal host instead of tmux, behind
an internal switch that no shipped supervisor turns on. Every session still starts in tmux until a later change turns
new terminals on; that change carries the real entry.
