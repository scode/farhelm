---
kind: fixed
---

`farhelm helm setup` no longer picks a tmux found through a relative `PATH` entry such as `.` or `node_modules/.bin`. Run from inside a directory holding a program named `tmux`, it used to write that program into the supervisor's boot-time service, so it ran every Farhelm session from then on. Setup now only considers absolute `PATH` entries, and when it finds no usable tmux it mentions any it skipped; pass `--tmux` with a path to use a specific tmux anyway.
