---
kind: fixed
---

When the tmux server could not be reached for a reason other than a missing session (a permission problem on its
socket, for example), the supervisor could mistake that for the session being gone if the path of its state directory
happened to contain words from tmux's "session not found" messages, and treat a live session as having lost its
terminal. It now reads tmux's own error message rather than a text that includes the path.
