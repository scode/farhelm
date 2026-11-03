---
kind: none
---

Nothing user-facing yet: this adds the supervisor's terminal emulator, which will interpret each new terminal's output
in place of tmux. It answers the program's terminal queries the way tmux does today, tracks the modes that typing,
pasting and the mouse depend on, and gives status detection the same screen text and title it reads from tmux now.
Nothing uses it until a later change turns new terminals on; that change carries the real entry.
