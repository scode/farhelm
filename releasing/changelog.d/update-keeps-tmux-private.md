---
kind: fixed
---

Updating a remote host no longer puts Farhelm's own tmux next to the `farhelm` binary. On hosts where `farhelm` lives in a directory on your PATH, such as `~/.local/bin` (where the install script puts it), Update installed Farhelm's tmux there as `tmux`, silently replacing your own tmux in that directory or putting Farhelm's first on your shell's PATH. It now always goes to `~/.local/lib/farhelm`. A `tmux` left in `~/.local/bin` by an earlier update is not removed; delete it by hand if you did not put it there.
