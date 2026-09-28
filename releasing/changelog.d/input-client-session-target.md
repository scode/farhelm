---
kind: none
---

Internal: the connection that carries a terminal's keystrokes now attaches to its session rather than to the terminal's pane, so opening a terminal tab no longer changes which window tmux considers the session's current one. Nothing Farhelm does depends on that today, so nothing visible changes.
