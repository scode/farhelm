---
kind: fixed
---

tmux commands run inside a session or terminal tab, such as `tmux new-window` or `tmux split-window`, now behave as they
would in a fresh SSH login: they reach your own tmux server, or report that none is running. Before, they reached the
hidden tmux that Farhelm runs sessions in, and nothing Farhelm did (Stop, Restart, Delete, closing the tab) cleaned up
what they started there. If your shell's startup files start or attach to tmux when they are not already inside tmux,
that now also happens in a new terminal tab, and in shells an agent starts for its own work, as it would in a terminal
outside tmux. Sessions and tabs that are already open keep the old behavior until they are restarted or reopened.
