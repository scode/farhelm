---
kind: fixed
---

`farhelm helm setup` and the desktop app's startup check no longer hang when the program they check as tmux (for example a wrapper script named `tmux`) starts a background process that detaches from it and keeps running. They now report that the program did not answer like tmux, as they already did for other misbehaving programs.
