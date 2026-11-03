---
kind: none
---

Nothing user-facing yet: the supervisor now knows what to do with a session whose agent runs in a terminal host at every
place it used to ask tmux (status and exit recording, Stop, Restart, Delete, prompts, attach, hook reports, and the
orderly stop's wait for new sessions). No session runs in a host until a later change turns new terminals on; that
change carries the real entry.
