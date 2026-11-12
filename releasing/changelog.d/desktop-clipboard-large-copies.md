---
kind: fixed
---

In the desktop app, a program's copy to the clipboard of more than 256 KiB of text now reaches the clipboard, up to 8
MiB, the most a terminal passes on. Before, such copies (an agent copying a long answer, say) were silently dropped in
the desktop app, while the same copy worked in a browser.

Copying a selection made with the mouse goes through the same route and gains the same room; a selection larger than 8
MiB is still dropped in the desktop app.
