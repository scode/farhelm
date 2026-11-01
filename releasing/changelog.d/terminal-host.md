---
kind: none
---

Nothing user-facing yet: this adds the wire protocol and the output journal of the terminal host, a small long-lived
process that will keep each new terminal's program running and its output journaled across supervisor restarts and
updates, replacing tmux for new terminals. Nothing uses them until later changes add the host itself and turn new
terminals on; that last change carries the real entry.
