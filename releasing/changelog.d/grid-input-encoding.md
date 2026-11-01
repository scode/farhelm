---
kind: none
---

Nothing user-facing yet: this adds the supervisor's side of typing into the coming terminals that do not run in tmux.
It turns what the user does in such a terminal (keys, text, pastes, the mouse, focus changes) into the bytes the program
reads, from the program's own terminal modes, so keys arrive exactly as they do in today's terminal (Shift+Enter still
inserts a newline in agents) and pastes are still marked as pastes. Nothing uses it until a later change turns those
terminals on; that change carries the real entry.
