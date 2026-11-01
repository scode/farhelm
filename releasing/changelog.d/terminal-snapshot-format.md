---
kind: none
---

Nothing user-facing yet: this adds the snapshot format the supervisor will use to rebuild a terminal's screen and
history after it restarts or updates, without the program noticing, and the history-only snapshot a restarted terminal
starts from. It also bounds how much memory one terminal's combining marks, underline colors and hyperlinks can take, and fixes stale background colors left
after clearing a resized full-screen program's screen. Nothing uses any of it until a later change turns new terminals
on; that change carries the real entry.
