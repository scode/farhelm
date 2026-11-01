---
kind: none
---

Nothing user-facing yet: this adds the terminal host, a small long-lived process (started as `farhelm internal
terminal-host`) that will keep each new terminal's program running and its output journaled across supervisor restarts
and updates, replacing tmux for new terminals. Nothing starts it until a later change turns new terminals on; that change
carries the real entry.
